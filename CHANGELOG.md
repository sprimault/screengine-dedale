# Journal des versions

La section en cours s'écrit `## [Non publié]`, sans date ni titre, et chaque lot y
ajoute ce qu'il change. **Elle prend son numéro et sa date au moment du tag.**

Chaque section est **bilingue, français d'abord, séparé par `***`**.

**Une étape franchie de `ROADMAP.md` se publie, en mineur : l'étape N donne la
`0.N.0`.** Sauf l'étape 0 — une version a un destinataire, et le squelette n'en a
aucun. La première est donc la `0.1.0`, à l'étape 1 : un labyrinthe qu'on parcourt,
cela s'adresse à quelqu'un.

## [Non publié]

### Ajouté
- **Le squelette du jeu** : la fenêtre s'ouvre, la boucle tourne, le moteur est lié
  par un commit épinglé, et l'interface se dessine dans le tampon de sortie — le
  seul endroit où elle a sa place.
- **La grille du labyrinthe**, engendrée à trois dimensions depuis une graine : une
  route et une seule entre deux cases, des étages reliés par des passages, et le
  même labyrinthe à chaque fois. Un plan la dessine à plat dans le tampon de
  sortie, le temps qu'un décor vienne à sa place.
- **La carte, écrite depuis la grille** : une cellule par case et une par passage,
  des murs qui ont une épaisseur, et aucun fichier intermédiaire — la graine est la
  forme de rejeu.
- **Le labyrinthe se parcourt** à la première personne, la cellule de la caméra
  suivie par les traversées de portails.

***

### Added
- **The game skeleton**: the window opens, the loop runs, the engine is linked by a
  pinned commit, and the interface draws into the output buffer — the only place it
  belongs.
- **The maze grid**, generated in three dimensions from a seed: one route and only
  one between two cells, floors linked by passages, and the same maze every time. A
  plan draws it flat into the output buffer, until scenery takes its place.
- **The map, written from the grid**: one cell per case and one per passage, walls
  that have a thickness, and no intermediate file — the seed is the form of replay.
- **The maze can be walked** in first person, the camera's cell followed through
  portal crossings.
