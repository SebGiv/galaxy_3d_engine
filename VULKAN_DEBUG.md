# Système de Debug Vulkan - Guide Complet

## Vue d'ensemble

Le système de debug Vulkan de Galaxy3D fournit un système de logging avancé avec statistiques, filtrage par catégorie, et tracking des messages en double.

## Fonctionnalités

### 1. Logging avec Couleurs
- **ERROR** : Rouge (messages d'erreur critiques)
- **WARNING** : Jaune (avertissements de performance ou d'utilisation)
- **INFO** : Cyan (informations générales)
- **VERBOSE** : Gris (détails verbeux pour débogage approfondi)

### 2. Filtrage par Sévérité

```rust
pub enum DebugSeverity {
    ErrorsOnly,          // Seulement les erreurs critiques
    ErrorsAndWarnings,   // Erreurs + Warnings (recommandé en développement)
    All,                 // Tous les messages (très détaillé)
}
```

### 3. Filtrage par Catégorie

```rust
pub struct DebugMessageFilter {
    pub show_general: bool,      // Messages généraux (création device, etc.)
    pub show_validation: bool,   // Erreurs d'utilisation de l'API
    pub show_performance: bool,  // Avertissements de performance
}
```

### 4. Destinations de Sortie

```rust
pub enum DebugOutput {
    Console,            // Console uniquement
    File(String),       // Fichier de log uniquement
    Both(String),       // Console + Fichier
}
```

### 5. Statistiques de Validation

Le système track automatiquement :
- Nombre d'erreurs
- Nombre de warnings
- Nombre de messages info
- Nombre de messages verbose
- Messages en double (avec compteur `[×N]`)

### 6. Modes Stricts pour le Développement

#### Break on Error
Arrête immédiatement l'exécution (pour attacher un debugger) :
```rust
break_on_validation_error: true
```

#### Panic on Error
Déclenche un panic avec stack trace complète :
```rust
panic_on_error: true
```

## Configuration Complète

### Exemple : Mode Développement Complet

```rust
use galaxy_3d_engine::{
    RendererConfig, DebugSeverity, DebugOutput, DebugMessageFilter
};

let config = RendererConfig {
    enable_validation: true,
    app_name: "Mon Application".to_string(),
    app_version: (1, 0, 0),

    // Afficher tous les messages
    debug_severity: DebugSeverity::All,

    // Console + fichier de log
    debug_output: DebugOutput::Both("vulkan_debug.log".to_string()),

    // Afficher toutes les catégories
    debug_message_filter: DebugMessageFilter {
        show_general: true,
        show_validation: true,
        show_performance: true,
    },

    // Ne pas arrêter sur erreur (laisser continuer pour voir toutes les erreurs)
    break_on_validation_error: false,

    // Ne pas panic (mode normal)
    panic_on_error: false,

    // Activer les statistiques
    enable_validation_stats: true,
};
```

### Exemple : Mode Production

```rust
let config = RendererConfig {
    enable_validation: false,  // Désactiver en production pour performance maximale
    app_name: "Mon Application".to_string(),
    app_version: (1, 0, 0),

    debug_severity: DebugSeverity::ErrorsOnly,
    debug_output: DebugOutput::File("errors.log".to_string()),

    debug_message_filter: DebugMessageFilter {
        show_general: false,
        show_validation: true,  // Garder validation en cas d'erreur critique
        show_performance: false,
    },

    break_on_validation_error: false,
    panic_on_error: false,
    enable_validation_stats: false,
};
```

### Exemple : Mode Debug Strict

```rust
let config = RendererConfig {
    enable_validation: true,
    app_name: "Mon Application".to_string(),
    app_version: (1, 0, 0),

    debug_severity: DebugSeverity::ErrorsAndWarnings,
    debug_output: DebugOutput::Console,

    debug_message_filter: DebugMessageFilter::default(),

    // Arrêter immédiatement sur erreur
    break_on_validation_error: true,

    // OU panic sur erreur (avec stack trace)
    panic_on_error: true,

    enable_validation_stats: true,
};
```

## Affichage des Statistiques

### API

```rust
use galaxy_3d_engine_renderer_vulkan::{
    get_validation_stats,
    print_validation_stats_report
};

// Obtenir les statistiques
let stats = get_validation_stats();
println!("Errors: {}", stats.errors);
println!("Warnings: {}", stats.warnings);
println!("Total: {}", stats.total());

// Afficher le rapport complet (coloré)
print_validation_stats_report();
```

### Rapport de Sortie

```
=== Validation Statistics Report ===
  Errors: 2
  Warnings: 5
  Info: 12
  Total: 19

  ℹ 3 message(s) appeared multiple times
====================================
```

## Format des Messages

### Console (avec couleurs)

```
[VULKAN ERROR] [Validation] [×2]
  ├─ Message ID: VUID-vkCmdDraw-None-02699
  └─ Buffer not bound before draw call
```

### Fichier (sans couleurs)

```
[VULKAN ERROR] [Validation] [×2]
  ├─ Message ID: VUID-vkCmdDraw-None-02699
  └─ Buffer not bound before draw call
```

Le `[×2]` indique que ce message est apparu 2 fois.

## Impact sur les Performances

### Mode Debug (enable_validation: true)
- **Overhead** : 50-200% de temps CPU supplémentaire
- **Usage** : Développement uniquement
- **Bénéfices** : Détecte bugs, memory leaks, erreurs d'API

### Mode Release (enable_validation: false)
- **Overhead** : ~0% (validation layers pas chargées)
- **Usage** : Production
- **Bénéfices** : Performance maximale

## Intégration avec l'Application

```rust
impl ApplicationHandler for App {
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Afficher le rapport avant de quitter
                galaxy_3d_engine_renderer_vulkan::print_validation_stats_report();

                // Cleanup...
                event_loop.exit();
            }
            _ => {}
        }
    }
}
```

## Fichier de Log

Le fichier de log est créé automatiquement si `DebugOutput::File` ou `DebugOutput::Both` est utilisé.

- **Format** : Texte sans couleurs (compatible avec éditeurs)
- **Mode** : Append (ajoute à la fin du fichier)
- **Contenu** : Même structure que console mais sans ANSI codes

## Conseils d'Utilisation

### Pendant le Développement
1. **enable_validation: true** - Toujours activer
2. **debug_severity: All** - Voir tous les messages
3. **enable_validation_stats: true** - Tracker les problèmes
4. **debug_output: Both** - Console + log pour revue ultérieure

### Pour Déboguer un Bug Spécifique
1. **break_on_validation_error: true** - Arrêter immédiatement
2. **panic_on_error: true** - Obtenir stack trace complète
3. Filtrer par catégorie pour réduire le bruit

### Avant la Release
1. **enable_validation: false** - Désactiver complètement
2. Vérifier que l'application fonctionne sans validation
3. Tester performance (devrait être identique à sans validation)

## Résolution de Problèmes Courants

### "Too many validation messages"
- Filtrer par catégorie : `show_performance: false`
- Augmenter sévérité : `DebugSeverity::ErrorsAndWarnings`

### "Can't find the error"
- Activer `break_on_validation_error` ou `panic_on_error`
- Le debugger s'arrêtera exactement où l'erreur se produit

### "Performance is slow"
- Vérifier `enable_validation` est `false` en release
- Les validation layers ont un overhead significatif

## API Complète

```rust
// Dans galaxy_3d_engine
pub struct RendererConfig {
    pub enable_validation: bool,
    pub debug_severity: DebugSeverity,
    pub debug_output: DebugOutput,
    pub debug_message_filter: DebugMessageFilter,
    pub break_on_validation_error: bool,
    pub panic_on_error: bool,
    pub enable_validation_stats: bool,
}

pub struct ValidationStats {
    pub errors: u32,
    pub warnings: u32,
    pub info: u32,
    pub verbose: u32,
}

impl ValidationStats {
    pub fn total(&self) -> u32;
    pub fn has_errors(&self) -> bool;
    pub fn has_warnings(&self) -> bool;
}

// Dans galaxy_3d_engine_renderer_vulkan
pub fn get_validation_stats() -> ValidationStats;
pub fn print_validation_stats_report();
```

## Voir Aussi

- [Vulkan Validation Layers](https://vulkan.lunarg.com/doc/sdk/latest/windows/khronos_validation_layer.html)
- [VUID Error Codes](https://registry.khronos.org/vulkan/specs/1.3/validusage/toc.html)
- Documentation Galaxy3D : `CLAUDE.md`
