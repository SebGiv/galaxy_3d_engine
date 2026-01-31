# Galaxy3DEngine - Règles de Développement

> **Project**: Multi-API 3D Rendering Engine in Rust
> **Date**: 2026-01-30

---

## 📋 Règles de Communication

### Langue de Communication

**TOUJOURS parler en français** avec l'utilisateur dans toutes les conversations.

**Exception** : Le code source, les commentaires dans le code, et les logs doivent être **en anglais**.

---

## ⚠️ En Cas de Doute ou d'Erreur

Si l'utilisateur te signale une violation de règle ou une erreur :

1. **STOP** - Ne pas deviner ou justifier
2. **RELIRE CLAUDE.md** immédiatement pour identifier l'erreur
3. **CORRIGER** en suivant la règle correcte

**Exemple** :
```
User: "Tu as violé une règle de CLAUDE.md"

Claude: [RELIRE IMMÉDIATEMENT CLAUDE.md avec l'outil Read]
Claude: "J'ai identifié l'erreur : [explication]. Je corrige maintenant en suivant la règle..."
```

**Règle importante** : En cas de doute sur n'importe quelle règle ou processus, toujours consulter CLAUDE.md en premier avant de répondre ou d'agir.

---

## 📁 Organisation des Fichiers

### Fichiers de Documentation

- **`CLAUDE.md`** (ce fichier) : Contient UNIQUEMENT les règles de développement du projet
- **`galaxy_3d_engine_dev.md`** : Contient TOUTES les analyses techniques, la planification des phases, et l'avancement du développement
  - **Référence principale** : Claude doit se référer à ce fichier pour continuer le développement même si la conversation précédente est perdue
  - **Mise à jour automatique** : Claude doit mettre à jour ce fichier automatiquement à chaque avancement ou analyse
  - **Langue** : Français

- **`doc/`** : Dossier contenant toute la documentation
  - **Documentation API HTML** :
    - `galaxy_3d_engine.html` : Documentation API en anglais
    - `galaxy_3d_engine_fr.html` : Documentation API en français
  - **Documentation Technique** :
    - `galaxy_3d_engine_tech_doc.md` : Documentation technique complète en anglais
    - `galaxy_3d_engine_tech_doc.fr.md` : Documentation technique complète en français
  - **Mise à jour automatique** : Claude doit mettre à jour TOUTES ces documentations au fur et à mesure du développement
  - **Référence principale** : Claude doit se référer au dossier `doc/` pour comprendre comment fonctionne le moteur

---

## 🔧 Règles de Développement

### 1. Avant Tout Développement (Codage, Résolution de Bug, etc.)

**RÈGLE IMPÉRATIVE** :

1. 📋 **CRÉER UNE TODO LIST avec l'outil TodoWrite** contenant OBLIGATOIREMENT :
   - Toutes les étapes de développement (création fichiers, modifications, tests, etc.)
   - ⚠️ **OBLIGATOIRE** : "Mettre à jour galaxy_3d_engine_dev.md"
   - ⚠️ **OBLIGATOIRE** : "Mettre à jour doc/galaxy_3d_engine.html" (si API publique change)
   - ⚠️ **OBLIGATOIRE** : "Mettre à jour doc/galaxy_3d_engine_fr.html" (si API publique change)
   - ⚠️ **OBLIGATOIRE** : "Mettre à jour doc/galaxy_3d_engine_tech_doc.md" (si architecture change)
   - ⚠️ **OBLIGATOIRE** : "Mettre à jour doc/galaxy_3d_engine_tech_doc.fr.md" (si architecture change)
   - Étape finale de commit

2. ✋ **Exposer clairement** ce qui va être fait (changements prévus, fichiers impactés, approche technique)

3. ⏸️ **STOP - ATTENDRE LE FEU VERT DE L'UTILISATEUR**
   - ⚠️ **CRITIQUE** : NE JAMAIS commencer le développement sans un "dev" ou "vas-y" explicite
   - ⚠️ Ceci s'applique MÊME si la TODO list a déjà été créée
   - ⚠️ Ceci s'applique MÊME si l'approche a été exposée
   - ⚠️ TOUJOURS attendre que l'utilisateur dise "dev" ou "vas-y"

4. ✅ Si l'utilisateur répond **"dev"** ou **"vas-y"** → Commencer le développement

5. ❌ Si l'utilisateur demande des modifications → Ajuster l'approche et re-exposer

**IMPORTANT** : L'utilisateur peut vérifier la TODO list et demander d'ajouter des étapes manquantes AVANT de dire "dev".

**Exemple** :
```
Claude: [Crée TODO list avec TodoWrite]
📋 TODO créée :
1. ⏳ Créer mesh_registry.rs
2. ⏳ Modifier renderer.rs : Ajouter create_global_buffers()
3. ⏳ Modifier vulkan_renderer.rs : Implémenter backend
4. ⏳ Mettre à jour galaxy_3d_engine_dev.md
5. ⏳ Mettre à jour doc/galaxy_3d_engine.html
6. ⏳ Mettre à jour doc/galaxy_3d_engine_fr.html
7. ⏳ Commit

Claude: "Je vais implémenter le mesh batching en modifiant les fichiers suivants :
- renderer.rs : Ajouter create_global_buffers()
- mesh_registry.rs : Créer nouvelle structure MeshRegistry
- vulkan_renderer.rs : Implémenter le backend Vulkan
Approche : [description technique]
Est-ce que je peux commencer le développement ?"

User: "dev"  ← Feu vert

Claude: [commence le développement en suivant la TODO]
1. 🔄 Créer mesh_registry.rs...
```

---

### 2. Avant Tout Commit/Push

**RÈGLE IMPÉRATIVE** :

1. ✋ **Exposer le message de commit** complet (titre + description) en **ANGLAIS**

2. ⏸️ **STOP - ATTENDRE LE FEU VERT DE L'UTILISATEUR POUR COMMIT/PUSH**
   - ⚠️ **CRITIQUE** : NE JAMAIS faire `git commit` sans feu vert explicite
   - ⚠️ **CRITIQUE** : NE JAMAIS faire `git push` sans feu vert explicite
   - ⚠️ Ceci s'applique MÊME si le développement est terminé
   - ⚠️ Ceci s'applique MÊME si les tests passent
   - ⚠️ TOUJOURS attendre que l'utilisateur dise "commit" ou "commit/push"

3. ✅ Si l'utilisateur répond **"commit"** → Faire `git commit` SEULEMENT (PAS de push)

4. ✅ Si l'utilisateur répond **"commit/push"** ou **"push"** → Faire `git commit` ET `git push`

5. ❌ Si l'utilisateur demande des modifications → Ajuster le message et re-exposer

**Langue des Messages de Commit** : **ANGLAIS** UNIQUEMENT

- ⚠️ **OBLIGATOIRE** : Titre en anglais
- ⚠️ **OBLIGATOIRE** : Description en anglais
- ⚠️ **OBLIGATOIRE** : Suivre les conventions Git standard (feat:, fix:, docs:, refactor:, etc.)
- ❌ **INTERDIT** : Aucun mot en français dans le message de commit

**Exemple** :
```
Claude: "Développement terminé. Je propose le message de commit suivant :

Titre: feat: Add mesh batching with global buffers

Description:
- Implement MeshRegistry for global vertex/index buffers
- Add create_global_buffers() to Renderer trait
- Update Vulkan backend to support batching
- Add example in galaxy3d_demo

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>

⏸️ STOP - J'attends ton feu vert pour commit/push."

User: "commit/push"  ← Feu vert pour commit ET push

Claude: [fait git commit ET git push]
```

---

### 3. Code Source et Logs

**Langue** : **Anglais** uniquement

**Commentaires dans le code** :
```rust
// ✅ CORRECT (English)
/// Creates a new mesh registry with global vertex and index buffers
pub fn create_mesh_registry(&self) -> Result<MeshRegistry> {
    // Allocate global buffers
    let vertex_buffer = self.create_buffer(...)?;
    // ...
}

// ❌ INCORRECT (Français)
/// Crée un nouveau registre de mesh avec des buffers globaux
pub fn create_mesh_registry(&self) -> Result<MeshRegistry> {
    // Allouer les buffers globaux
    let vertex_buffer = self.create_buffer(...)?;
    // ...
}
```

**Logs** :
```rust
// ✅ CORRECT (English)
log::info!("Mesh registry created with {} meshes", count);
log::error!("Failed to allocate global vertex buffer: {}", err);

// ❌ INCORRECT (Français)
log::info!("Registre de mesh créé avec {} meshes", count);
log::error!("Échec d'allocation du buffer vertex global: {}", err);
```

---

## 📚 Documentation HTML

### Structure de la Documentation

La documentation se trouve dans le dossier **`doc/`** :
- **`doc/galaxy_3d_engine.html`** : Version anglaise
- **`doc/galaxy_3d_engine_fr.html`** : Version française

### Format de la Documentation

**Organisation** :
- 📑 **Table des matières cliquable** avec sous-rubriques logiques
- 📦 **Une rubrique par structure** + ensemble de fonctions publiques liées
- 🔗 **Lien vers table des matières** au début de chaque rubrique
- 📂 **Regroupement logique** (ex: tout le Renderer ensemble, tous les objets liés au Renderer groupés)

**Contenu de chaque élément public** :
- **Nom** de la structure/fonction/méthode
- **Description succincte** (1-2 lignes)
- **Clic** → Ouvre un **accordéon** contenant :
  - Description complète de l'utilisation
  - Exemple de code complet

**Exemple de structure** :
```html
<!-- Table des matières -->
<nav id="toc">
  <h2>Table des Matières</h2>
  <ul>
    <li><a href="#renderer">Renderer</a>
      <ul>
        <li><a href="#renderer-creation">Creation & Initialization</a></li>
        <li><a href="#renderer-resources">Resource Management</a></li>
        <li><a href="#renderer-rendering">Rendering</a></li>
      </ul>
    </li>
    <li><a href="#command-list">Command List</a></li>
    <!-- ... -->
  </ul>
</nav>

<!-- Rubrique Renderer -->
<section id="renderer">
  <a href="#toc">↑ Table des Matières</a>
  <h2>Renderer</h2>

  <div class="api-item">
    <h3 onclick="toggleAccordion('renderer-new')">
      Renderer::new()
      <span class="summary">Creates a new renderer instance</span>
    </h3>
    <div id="renderer-new" class="accordion-content">
      <p>Detailed description...</p>
      <pre><code class="language-rust">
// Example code
let renderer = VulkanRenderer::new(&window, config)?;
      </code></pre>
    </div>
  </div>

  <!-- ... autres éléments ... -->
</section>
```

**Organisation du Contenu** :

La documentation HTML suit cette structure :

1. **Section Renderer** (Factory/Device)
   - Contient TOUTES les méthodes de création avec descriptions complètes
   - `create_buffer()`, `create_texture()`, `create_shader()`, etc.
   - Chaque méthode a : description, paramètres, retour, exemple de code

2. **Sections par Type de Ressource** (Buffer, Texture, Shader, etc.)
   - **Lien vers Renderer** : Référence vers la méthode `create_xxx()` dans Renderer
   - **Trait Public** : Documentation du trait avec toutes ses méthodes publiques
   - **Exemples d'utilisation** : Code montrant comment utiliser le trait

**Exemple de structure** :
```
Buffer
├── "See Renderer::create_buffer() for creation" (lien)
└── RendererBuffer Trait
    └── update() - Description + exemple

Texture
├── "See Renderer::create_texture() for creation" (lien)
└── RendererTexture Trait
    └── (No public methods - Marker trait)
```

**Avantages** :
- ✅ Deux chemins d'accès (création dans Renderer, utilisation dans section dédiée)
- ✅ Pas de duplication du contenu
- ✅ Facile à trouver ce qu'on cherche

**Mise à jour** :
- ♻️ **Automatique** : Claude doit mettre à jour la documentation HTML au fur et à mesure du développement du moteur
- 📝 Ajouter les nouvelles structures/fonctions dès qu'elles sont implémentées
- 🔄 Mettre à jour les exemples si l'API change
- 🔗 Maintenir les liens entre sections (Renderer ↔ Traits)

---

## 📖 Documentation Technique

### Structure de la Documentation Technique

La documentation technique se trouve dans le dossier **`doc/`** :
- **`doc/galaxy_3d_engine_tech_doc.md`** : Version anglaise
- **`doc/galaxy_3d_engine_tech_doc.fr.md`** : Version française

### Contenu de la Documentation Technique

La documentation technique est une référence complète et détaillée de l'architecture du moteur :

**Architecture & Design** :
- Vue d'ensemble de l'architecture multi-crates
- Principes de conception fondamentaux
- Hiérarchie des traits
- Patterns de design utilisés

**Implémentation** :
- Gestion des ressources (buffers, textures, shaders, pipelines)
- Pipeline de rendu complet
- Détails d'implémentation du backend Vulkan
- Synchronisation CPU-GPU
- Gestion mémoire GPU (gpu-allocator)

**Références Techniques** :
- Descripteurs de ressources (BufferDesc, TextureDesc, etc.)
- API complète de tous les traits
- Exemples de code d'utilisation
- Flux d'exécution détaillés

**Extensibilité** :
- Features plannifiées (Phases 10+)
- Support multi-backend (D3D12, Metal)
- Points d'extension

### Utilisation par Claude

**RÈGLE IMPORTANTE** :

Claude doit **toujours consulter le dossier `doc/`** pour :
- ✅ Comprendre comment fonctionne le moteur
- ✅ Vérifier l'architecture existante avant de proposer des changements
- ✅ S'assurer de la cohérence avec les design patterns utilisés
- ✅ Référencer les structures et traits déjà implémentés

**Avant toute modification** :
1. Lire la documentation technique pertinente dans `doc/`
2. Comprendre l'architecture actuelle
3. Proposer des changements cohérents avec le design existant
4. Mettre à jour la documentation après implémentation

### Mise à Jour de la Documentation Technique

**Quand mettre à jour** :
- ✨ Après l'ajout d'une nouvelle feature majeure
- 🔄 Après modification d'une API existante
- 📦 Après ajout de nouveaux traits/structures
- 🏗️ Après changement architectural

**Comment mettre à jour** :
1. **Identifier les sections impactées** dans les deux versions (EN + FR)
2. **Mettre à jour la version anglaise** (`galaxy_3d_engine_tech_doc.md`)
3. **Mettre à jour la version française** (`galaxy_3d_engine_tech_doc.fr.md`)
4. **Vérifier la cohérence** entre les deux versions
5. **Ajouter des exemples de code** si nécessaire

**Sections à maintenir** :
- Table des matières (à jour avec nouvelles sections)
- Architecture Overview (si changements structurels)
- Trait Hierarchy (si nouveaux traits)
- Resource Management (si nouveaux types de ressources)
- Rendering Pipeline (si nouveau flux)
- API Reference Summary (toujours à jour)

---

## 🎯 Workflow de Développement

### Workflow Type pour une Nouvelle Feature

1. **Analyse et Planification**
   - Discuter de la feature avec l'utilisateur
   - Mettre à jour `galaxy_3d_engine_dev.md` avec l'analyse technique

2. **Proposition de Développement**
   - Exposer les changements prévus
   - Attendre le feu vert ("dev")

3. **Développement**
   - Coder la feature (code + commentaires en anglais)
   - Mettre à jour `galaxy_3d_engine_dev.md` avec l'avancement

4. **Documentation**
   - Mettre à jour `doc/galaxy_3d_engine.html` (EN) - Documentation API
   - Mettre à jour `doc/galaxy_3d_engine_fr.html` (FR) - Documentation API
   - Mettre à jour `doc/galaxy_3d_engine_tech_doc.md` (EN) - Documentation technique
   - Mettre à jour `doc/galaxy_3d_engine_tech_doc.fr.md` (FR) - Documentation technique

5. **Commit**
   - Exposer le message de commit
   - Attendre le feu vert ("commit" ou "commit/push")
   - Commit/push selon l'instruction

---

## 📖 Référence Rapide

| Situation | Action Claude | Attente User |
|-----------|---------------|--------------|
| Avant dev | Exposer les changements prévus | "dev" / "vas-y" |
| Avant commit | Exposer le message de commit | "commit" / "commit/push" |
| Code source | Écrire en anglais (commentaires + logs) | - |
| Conversation | Parler en français | - |
| Mise à jour doc | Automatique après chaque feature | - |
| Référence technique | Consulter `doc/` (tech doc) et `galaxy_3d_engine_dev.md` | - |
| Comprendre le moteur | Lire `doc/galaxy_3d_engine_tech_doc.md` | - |

---

## ✅ Checklist Avant Chaque Action

### Avant de Coder
- [ ] J'ai exposé clairement ce que je vais faire
- [ ] J'ai attendu le feu vert de l'utilisateur
- [ ] Je vais écrire le code et les commentaires en anglais

### Avant de Commit
- [ ] J'ai exposé le message de commit complet
- [ ] J'ai attendu l'instruction ("commit" ou "commit/push")
- [ ] Je vais suivre l'instruction exactement

### Après Développement
- [ ] J'ai mis à jour `galaxy_3d_engine_dev.md`
- [ ] J'ai mis à jour la documentation HTML API (EN + FR)
- [ ] J'ai mis à jour la documentation technique (EN + FR) si nécessaire
- [ ] Les logs sont en anglais
- [ ] J'ai consulté `doc/` pour vérifier la cohérence

---

**Note** : Ces règles sont **impératives** et doivent être suivies à chaque fois, sans exception.
