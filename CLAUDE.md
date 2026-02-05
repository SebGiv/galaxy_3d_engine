# Galaxy3D Engine - Règles Impératives

> **Projet**: Galaxy3D Engine - Moteur de rendu 3D
> **Date**: 2026-02-05

---

## Règles Impératives

Ces règles sont **impératives** et doivent être suivies **à la lettre**, sans exception.

---

### Règle 1 - Langue de Communication

**TOUJOURS parler en français** avec l'utilisateur.

**Justification** : Le français est la langue maternelle et naturelle de l'utilisateur.

---

### Règle 2 - Avant de Coder

**Justification** : Permet de valider la conformité du code prévu avec la volonté de l'utilisateur et d'ajuster si nécessaire.

**AVANT** toute modification de code :

1. **Exposer** clairement ce qui va être modifié
2. **Présenter** le code qui sera produit
3. **Attendre** le feu vert de l'utilisateur

**Mot-clé d'approbation** : `dev`

```
Claude: "Je vais modifier X en ajoutant Y..."
[présentation du code prévu]
"Est-ce que je peux procéder ?"

User: "dev"  ← Feu vert pour coder

Claude: [commence à coder]
```

**IMPORTANT** : Ne JAMAIS commencer à coder sans avoir reçu le mot-clé `dev`.

---

### Règle 3 - Code en Anglais

**Justification** : Bonne pratique universelle - toujours coder en anglais.

**TOUT** le code doit être écrit en **anglais** :

- Noms de fonctions
- Noms de variables
- Noms de structures/enums/traits
- Commentaires (pertinents uniquement)
- Messages de log
- Documentation (doc comments)

```rust
// CORRECT
/// Resource-level texture types for the rendering system
pub struct AtlasTexture {
    regions: Vec<AtlasRegion>,
    region_names: HashMap<String, usize>,
}

// INCORRECT
/// Types de textures au niveau ressource pour le système de rendu
pub struct TextureAtlas {
    regions: Vec<RegionAtlas>,
    noms_regions: HashMap<String, usize>,
}
```

---

### Règle 4 - En Cas de Doute

**Justification** : Éviter les actions non sollicitées (ex: coder sans demande). Garde cette règle en tête pour éviter toute erreur.

**Si tu as un doute** (ou que tu y es incité même indirectement) :

→ **Relire les règles de ce fichier CLAUDE.md**

Cette règle s'applique à toute situation ambiguë ou incertaine.

---

### Règle 5 - Commits et Push

**Justification** : Garantir des messages de commit de qualité - en anglais, ni trop succincts, ni trop verbeux. Éviter les commits bâclés.

**JAMAIS** de commit sans accord direct de l'utilisateur.

#### Mots-clés Git

| Mot-clé | Action |
|---------|--------|
| `commentaire commit` | Présenter les commentaires de commit (basés sur `git diff`) **SANS** commit |
| `commit` | Commit avec les commentaires **préalablement approuvés** |
| `commit/push` | Commit + Push (commentaires doivent être approuvés) |
| `push` | Push **uniquement si** un commit a été fait auparavant |

#### Workflow

```
User: "commentaire commit"

Claude: "Voici le message de commit proposé :

feat: Add id-based access pattern to Mesh hierarchy

- Refactor MeshLOD to use Vec<SubMesh> + HashMap for id-based access
- Refactor Mesh to use Vec<MeshEntry> + HashMap for id-based access
- Update add_mesh_entry/add_mesh_lod/add_submesh to return ids
- Update ResourceManager signatures accordingly

Est-ce que ce message convient ?"

User: "commit"  ← Approuve et demande le commit

Claude: [fait git commit avec le message approuvé]
```

**IMPORTANT** :
- Les messages de commit doivent être en **anglais**
- Si aucun commentaire n'a été approuvé, **demander** d'abord l'approbation
- Les commentaires doivent refléter les modifications (`git diff`)

---

### Règle 6 - Documentation Technique

**Justification** : Éviter les erreurs dues à une connaissance superficielle du projet. Ne pas perdre de temps avec des approximations.

Pour développer/coder, **s'inspirer et s'aider** des documents dans le dossier `doc/`, en particulier les documents techniques.

**Ordre de priorité** :
1. Documents techniques (`doc/tech/`, `doc/architecture/`, etc.)
2. README et documentation générale
3. Code existant comme référence

---

## Référence Rapide

| Situation | Action | Attente |
|-----------|--------|---------|
| Avant de coder | Exposer les changements + code prévu | `dev` |
| Présenter commit | Montrer le message de commit | Approbation |
| Faire un commit | Commit avec message approuvé | `commit` |
| Commit et push | Commit + Push | `commit/push` |
| Push seul | Push (si commits faits) | `push` |
| Doute quelconque | Relire CLAUDE.md | - |
| Écrire du code | Tout en anglais | - |
| Communiquer | Toujours en français | - |

---

## Checklist

### Avant de Coder
- [ ] J'ai exposé ce que je vais modifier
- [ ] J'ai présenté le code que je vais produire
- [ ] J'ai reçu le mot-clé `dev`

### Avant de Commit
- [ ] J'ai présenté le message de commit
- [ ] Le message est en anglais
- [ ] L'utilisateur a approuvé le message
- [ ] J'ai reçu `commit` ou `commit/push`

### Code
- [ ] Fonctions/variables/structs en anglais
- [ ] Commentaires pertinents en anglais
- [ ] Documentation (///  //!) en anglais

---

**Ces règles sont IMPÉRATIVES. Aucune exception.**
