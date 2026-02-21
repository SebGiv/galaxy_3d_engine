/// Resource-level structured GPU buffer (UBO or SSBO).
///
/// A Buffer wraps a renderer::Buffer with field metadata describing
/// its internal structure. It knows the layout of each element
/// and can provide safe or unsafe access to individual elements.
///
/// Architecture:
/// - BufferKind: Uniform (UBO) or Storage (SSBO)
/// - Fields: ordered list of named typed fields (e.g. "world" → Mat4)
/// - Count: number of elements (array of structures)
/// - Layout computed automatically (std140 for UBO, std430 for SSBO)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::renderer::{self, Buffer as RendererBuffer};

// ===== BUFFER KIND =====

/// Uniform (UBO) or Storage (SSBO)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferKind {
    Uniform,
    Storage,
}

// ===== FIELD TYPE =====

/// Data type for a field within the buffer structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat3,
    Mat4,
    Int,
    UInt,
}

impl FieldType {
    /// Size in bytes (std140 layout rules)
    pub fn size_bytes(&self) -> u64 {
        match self {
            FieldType::Float => 4,
            FieldType::Vec2  => 8,
            FieldType::Vec3  => 16, // std140: vec3 padded to 16
            FieldType::Vec4  => 16,
            FieldType::Mat3  => 48, // std140: 3 × vec4
            FieldType::Mat4  => 64,
            FieldType::Int   => 4,
            FieldType::UInt  => 4,
        }
    }

    /// Alignment in bytes (std140 layout rules)
    pub fn alignment(&self) -> u64 {
        match self {
            FieldType::Float => 4,
            FieldType::Vec2  => 8,
            FieldType::Vec3  => 16,
            FieldType::Vec4  => 16,
            FieldType::Mat3  => 16,
            FieldType::Mat4  => 16,
            FieldType::Int   => 4,
            FieldType::UInt  => 4,
        }
    }
}

// ===== FIELD DESC =====

/// A named field in the buffer structure
#[derive(Debug, Clone)]
pub struct FieldDesc {
    pub name: String,
    pub field_type: FieldType,
}

// ===== BUFFER DESC =====

/// Descriptor for creating a resource::Buffer
pub struct BufferDesc {
    pub renderer: Arc<Mutex<dyn renderer::Renderer>>,
    pub kind: BufferKind,
    pub fields: Vec<FieldDesc>,
    pub count: u32,
}

// ===== BUFFER =====

/// GPU buffer resource with structured layout
///
/// Wraps a renderer::Buffer (UBO or SSBO) with field metadata.
/// Knows its internal structure and can provide safe update methods
/// or unsafe direct pointer access for performance-critical paths.
pub struct Buffer {
    renderer_buffer: Arc<dyn RendererBuffer>,
    kind: BufferKind,
    fields: Vec<FieldDesc>,
    field_names: HashMap<String, usize>,
    field_offsets: Vec<u64>,
    stride: u64,
    count: u32,
    size: u64,
}

impl Buffer {
    pub(crate) fn from_desc(desc: BufferDesc) -> Result<Self> {
        // ========== VALIDATION ==========
        if desc.fields.is_empty() {
            engine_bail!("galaxy3d::Buffer", "Buffer must have at least one field");
        }
        if desc.count == 0 {
            engine_bail!("galaxy3d::Buffer", "Buffer must have at least one element");
        }

        let mut seen = std::collections::HashSet::new();
        for field in &desc.fields {
            if !seen.insert(&field.name) {
                engine_bail!("galaxy3d::Buffer",
                    "Duplicate field name '{}'", field.name);
            }
        }

        // ========== COMPUTE LAYOUT ==========
        let mut field_offsets = Vec::with_capacity(desc.fields.len());
        let mut field_names = HashMap::new();
        let mut current_offset: u64 = 0;

        for (index, field) in desc.fields.iter().enumerate() {
            let align = field.field_type.alignment();
            // Align current offset
            current_offset = (current_offset + align - 1) & !(align - 1);
            field_offsets.push(current_offset);
            field_names.insert(field.name.clone(), index);
            current_offset += field.field_type.size_bytes();
        }

        // Stride: structure size aligned according to layout rules
        // - Uniform (UBO) → std140: struct alignment is at least 16
        // - Storage (SSBO) → std430: natural alignment (max of field alignments)
        let max_field_align: u64 = desc.fields.iter()
            .map(|f| f.field_type.alignment())
            .max()
            .unwrap_or(4);

        let struct_align: u64 = match desc.kind {
            BufferKind::Uniform => max_field_align.max(16),
            BufferKind::Storage => max_field_align,
        };

        let stride = (current_offset + struct_align - 1) & !(struct_align - 1);
        let size = stride * desc.count as u64;

        // ========== CREATE GPU BUFFER ==========
        let usage = match desc.kind {
            BufferKind::Uniform => renderer::BufferUsage::Uniform,
            BufferKind::Storage => renderer::BufferUsage::Storage,
        };

        let renderer_buffer = desc.renderer.lock().unwrap()
            .create_buffer(renderer::BufferDesc { size, usage })?;

        Ok(Self {
            renderer_buffer,
            kind: desc.kind,
            fields: desc.fields,
            field_names,
            field_offsets,
            stride,
            count: desc.count,
            size,
        })
    }

    // ===== ACCESSORS =====

    /// Get buffer kind (Uniform or Storage)
    pub fn kind(&self) -> BufferKind { self.kind }

    /// Get stride in bytes (size of one element, aligned)
    pub fn stride(&self) -> u64 { self.stride }

    /// Get number of elements
    pub fn count(&self) -> u32 { self.count }

    /// Get total buffer size in bytes
    pub fn size(&self) -> u64 { self.size }

    /// Get field descriptors
    pub fn fields(&self) -> &[FieldDesc] { &self.fields }

    /// Get the underlying renderer buffer
    pub fn renderer_buffer(&self) -> &Arc<dyn RendererBuffer> { &self.renderer_buffer }

    /// Get field index by name
    pub fn field_id(&self, name: &str) -> Option<usize> {
        self.field_names.get(name).copied()
    }

    /// Get field offset within the structure (bytes)
    pub fn field_offset(&self, field_index: usize) -> Option<u64> {
        self.field_offsets.get(field_index).copied()
    }

    // ===== SAFE UPDATE METHODS =====

    /// Update a whole element at index
    pub fn update_element(&self, index: u32, data: &[u8]) -> Result<()> {
        if index >= self.count {
            engine_bail!("galaxy3d::Buffer",
                "Element index {} out of bounds (count: {})", index, self.count);
        }
        if data.len() as u64 > self.stride {
            engine_bail!("galaxy3d::Buffer",
                "Data size {} exceeds stride {}", data.len(), self.stride);
        }
        let offset = self.stride * index as u64;
        self.renderer_buffer.update(offset, data)
    }

    /// Update a specific field of a specific element
    pub fn update_field(&self, index: u32, field_index: usize, data: &[u8]) -> Result<()> {
        if index >= self.count {
            engine_bail!("galaxy3d::Buffer",
                "Element index {} out of bounds (count: {})", index, self.count);
        }
        let field_offset = self.field_offsets.get(field_index)
            .ok_or_else(|| engine_err!("galaxy3d::Buffer",
                "Field index {} out of bounds", field_index))?;
        let field_size = self.fields[field_index].field_type.size_bytes();
        if data.len() as u64 != field_size {
            engine_bail!("galaxy3d::Buffer",
                "Data size {} doesn't match field size {}", data.len(), field_size);
        }
        let offset = self.stride * index as u64 + field_offset;
        self.renderer_buffer.update(offset, data)
    }

    /// Update raw bytes at arbitrary offset
    pub fn update_raw(&self, offset: u64, data: &[u8]) -> Result<()> {
        if offset + data.len() as u64 > self.size {
            engine_bail!("galaxy3d::Buffer",
                "Write at offset {} with size {} exceeds buffer size {}",
                offset, data.len(), self.size);
        }
        self.renderer_buffer.update(offset, data)
    }

    // ===== UNSAFE DIRECT ACCESS =====

    /// Raw pointer to the beginning of the mapped buffer
    ///
    /// # Safety
    ///
    /// - No synchronization with GPU reads — caller must ensure
    ///   the GPU is not reading this buffer (e.g. double buffering)
    /// - No bounds checking on subsequent writes
    pub unsafe fn buffer_ptr(&self) -> Option<*mut u8> {
        self.renderer_buffer.mapped_ptr()
    }

    /// Raw pointer to the start of element at index
    ///
    /// # Safety
    ///
    /// - No synchronization with GPU reads
    /// - No bounds checking on subsequent writes
    /// - Caller must ensure index < count
    pub unsafe fn element_ptr(&self, index: u32) -> Option<*mut u8> {
        self.renderer_buffer.mapped_ptr()
            .map(|base| base.add((self.stride * index as u64) as usize))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
