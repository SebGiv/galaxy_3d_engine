# Documentation d'Utilisation - Règles Impératives

> **Fichiers concernés** : `galaxy_3d_engine.html` (EN) et `galaxy_3d_engine_fr.html` (FR)
> **Date** : 2026-02-05

---

## Règles Impératives

Ces règles sont **impératives** et doivent être suivies **à la lettre** lors de toute modification des fichiers de documentation.

---

### Règle 1 - Mise à Jour sur Demande Uniquement

**JAMAIS** de mise à jour automatique de la documentation.

Toutes les documentations du dossier `doc/` sont mises à jour **uniquement sur demande explicite** de l'utilisateur.

---

### Règle 2 - Structure Identique EN/FR

Les deux fichiers HTML doivent avoir une **structure strictement identique** :
- `galaxy_3d_engine.html` — Version anglaise
- `galaxy_3d_engine_fr.html` — Version française

Toute modification dans l'un doit être répercutée dans l'autre.

---

### Règle 3 - Interface Utilisateur

**Table des matières** :
- Positionnée à **gauche**, toujours visible
- Chapitres et sous-chapitres avec **accordéons** (ouvrir/fermer)
- **Ascenseur** (scroll) si le contenu dépasse la hauteur disponible

**Contenu API** :
- Utiliser des **accordéons interactifs** pour chaque élément d'API

---

### Règle 4 - Contenu Public Uniquement

**Représenter uniquement** les structures et fonctions **publiques** destinées à l'utilisateur.

Ne pas documenter :
- Les éléments internes (`pub(crate)`, `pub(super)`)
- Les détails d'implémentation privés
- Les helpers internes

---

### Règle 5 - Organisation par Concepts

**Structurer** la table des matières et le contenu **par concepts du projet** :

| Concept | Contenu |
|---------|---------|
| `render::` | Tous les objets du module render (Renderer, Buffer, Texture, etc.) |
| `resource::` | Tous les objets du module resource (ResourceManager, Mesh, Texture, etc.) |
| Autres modules | Regroupés de manière cohérente |

La documentation doit refléter l'architecture conceptuelle du projet, pas une liste alphabétique.

---

### Règle 6 - Encodage HTML

Utiliser les entités HTML pour les caractères spéciaux dans le code :

| Caractère | Encodage |
|-----------|----------|
| `<` | `&lt;` |
| `>` | `&gt;` |
| `&` | `&amp;` |

---

**Ces règles sont IMPÉRATIVES. Aucune exception.**
