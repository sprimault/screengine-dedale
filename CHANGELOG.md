# Journal des versions

La section en cours s'écrit `## [Non publié]`, sans date ni titre, et chaque lot y
ajoute ce qu'il change : écrite au fil de l'eau, elle est relue en même temps que
le code qu'elle décrit, alors qu'une section rédigée le jour du tag résume de
mémoire un mois de travail. **Elle prend son numéro et sa date au moment du tag.**

Chaque section est **bilingue, français d'abord, séparé par `***`**. Ce préambule
reste en français : il n'est jamais publié, et explique les conventions du dépôt à
qui y contribue.

**Une entrée tient en cinq lignes par langue, jamais plus.** Elle dit ce qui
change pour quelqu'un qui joue ou qui construit, jamais le mécanisme — celui-là est
déjà dans le code, là où on le relira avec lui.

**Une étape franchie de `ROADMAP.md` se publie**, en mineur : l'étape N donne la
`0.N.0`. Entre deux étapes, fusionner une PR ne justifie pas de publier.

**En `0.x`, le numéro ne prévient de rien.** Ce qui doit se dire en tête d'une
section, quand c'est le cas :

- une sauvegarde de partie qui ne se relit plus ;
- un changement de format de carte engendrée, si le jeu en garde ;
- la version du moteur contre laquelle cette version est écrite, quand elle change.

## [Non publié]

### Ajouté
- **Le squelette du jeu** : la fenêtre s'ouvre, la boucle tourne, le moteur est lié
  par un commit épinglé, et une jauge se dessine dans le tampon de sortie — le seul
  endroit où une interface a sa place.

***

### Added
- **The game skeleton**: the window opens, the loop runs, the engine is linked by a
  pinned commit, and a gauge draws into the output buffer — the only place an
  interface belongs.
