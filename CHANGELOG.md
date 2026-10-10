# Journal des versions

La section en cours s'écrit `## [Non publié]`, sans date ni titre, et chaque lot y
ajoute ce qu'il change. **Elle prend son numéro et sa date au moment du tag**, et une
nouvelle section vide s'ouvre aussitôt au-dessus : le lot suivant trouve où écrire
sans avoir à y penser.

Chaque section est **bilingue, français d'abord, séparé par `***`**.

**Une étape franchie de `ROADMAP.md` se publie, en mineur : l'étape N donne la
`0.N.0`.** Sauf l'étape 0 — une version a un destinataire, et le squelette n'en a
aucun. La première est donc la `0.1.0`, à l'étape 1 : un labyrinthe qu'on parcourt,
cela s'adresse à quelqu'un.

## [Non publié]

### Ajouté
- **Les démons blessent au contact**, et une barre de vie en bas de l'écran dit ce
  qu'il reste. À zéro la partie se fige, et un texte dit que `R` en relance une.
- **Un démon abattu compte**, chaque silhouette pour sa valeur, et le score se lit en
  bas à droite.

### Corrigé
- **Un démon se touche là où on le voit** : viser le flanc des plus larges ratait le
  coup.

***

### Added
- **Demons hurt on contact**, and a health bar at the bottom of the screen shows what
  is left. At zero the game freezes, and a line says `R` starts a new one.
- **Downing a demon scores**, each figure for its own value, and the count reads at the
  bottom right.

### Fixed
- **A demon is hit where it is seen**: aiming at the flank of the wider ones missed.

## [0.4.0] — 2026-10-07 — Le tir et le contact

### Ajouté
- **On tire, et ça porte** : les murs gardent la marque des impacts, et les démons
  encaissent les coups avant de tomber au troisième sans rien laisser.
- **Un réticule marque le centre de la vue**, là où le tir part — et l'arme s'aligne
  dessus.

***

### Added
- **Shots land**: walls keep the mark of each impact, and demons take the hits before
  going down on the third, leaving nothing behind.
- **A reticle marks the centre of the view**, where the shot starts — and the weapon
  lines up with it.

## [0.3.0] — 2026-10-06 — Les monstres

### Ajouté
- **Trois démons parcourent le labyrinthe**, chacun sa silhouette : ils posent leur
  ombre au sol, s'évitent, et se montrent sous huit angles selon d'où on les regarde.

***

### Added
- **Three demons roam the maze**, each with its own figure: they cast their shadow
  on the floor, keep out of each other's way, and are shown from eight angles.

## [0.2.2] — 2026-10-05 — Les règles du moteur, et pas les nôtres

### Modifié
- **Rien ne change pour qui joue.** Le jeu ne juge plus ses cartes sur ses
  propres règles, mais sur celles du moteur, seul à en faire foi.

***

### Changed
- **Nothing changes for players.** The game no longer judges its maps by its own
  rules, but by the engine's, which alone are authoritative.

## [0.2.1] — 2026-10-05 — Le plan juste et les planches signées

### Modifié
- **Chaque planche porte son auteur et sa licence**, relus dans le fichier
  d'image lui-même.

### Corrigé
- **Le repère d'une rampe pointait du mauvais côté** sur le plan affiché au coin
  de l'écran : il montrait le pied de la pente au lieu de sa montée.

***

### Changed
- **Every sheet carries its author and licence**, read back from the image file
  itself.

### Fixed
- **A ramp's marker pointed the wrong way** on the map in the corner of the
  screen: it showed the foot of the slope instead of the way up.

## [0.2.0] — 2026-10-04 — On marche dans le labyrinthe

### Ajouté
- **On marche** : le décor arrête, on longe les murs, on monte les marches et les
  rampes, et la pesanteur ramène au sol. Un pas de côté sur `A` et `E`.
- **L'arme en main**, qui balance au rythme des pas et penche dans les virages. Le
  clic droit la fait tirer — la pose seule, pour l'instant.

### Modifié
- **Les étages sont plus hauts** d'un demi-mètre : un couloir large de trois mètres
  sous deux et demi paraissait écrasé.

### Corrigé
- **L'image ne se vide plus en franchissant une porte.** De près, le décor au-delà
  d'une embrasure disparaissait le temps d'une ou deux images.

***

### Added
- **You walk**: the map stops you, you hug walls, you climb steps and ramps, and
  gravity brings you back down. Strafing on `Q` and `E`.
- **The weapon in hand**, swaying with your stride and leaning into turns. Right
  click fires it — the pose alone, for now.

### Changed
- **Floors are half a metre taller**: a corridor three metres wide under two and a
  half looked squashed.

### Fixed
- **The view no longer empties as you step through a doorway.** Up close, the
  scenery beyond it vanished for a frame or two.

## [0.1.1] — 2026-10-03 — Des rampes dans les cages

### Ajouté
- **Des rampes dans une partie des cages**, à côté des escaliers : on y monte
  d'une seule pente, et elles se tirent avec la graine comme le reste du décor.

***

### Added
- **Ramps in some of the shafts**, alongside the stairs: a single slope to walk
  up, drawn from the seed like the rest of the map.

## [0.1.0] — 2026-10-02 — Un labyrinthe qu'on parcourt

### Ajouté
- **Un labyrinthe qu'on parcourt**, à la première personne : des étages reliés par
  des cages d'escalier, et une graine qui rejoue le même.
- **Le décor est engendré, jamais dessiné à la main** : la grille devient les
  cellules et les portails que le moteur charge, avec son éclairage et ses repères.

***

### Added
- **A maze you walk**, in first person: floors linked by stairwells, and a seed that
  plays the same one again.
- **The map is generated, never drawn by hand**: the grid becomes the cells and
  portals the engine loads, with its lighting and its markers.
