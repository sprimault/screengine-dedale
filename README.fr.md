# Dédale

English: [README.md](README.md)

Sortir d'un labyrinthe. On y ramasse des munitions et des fioles de soin, des
monstres le parcourent, il faut les abattre ou perdre de la vie à leur contact.

**C'est le jeu de démonstration de [screengine](https://github.com/sprimault/screengine)**,
un moteur de rendu 3D logiciel : pas de GPU, couleurs directes, lightmaps, et la
même image au bit près sur toutes les cibles. Le jeu existe pour montrer ce que le
moteur fait, et pour l'éprouver là où ses propres exemples ne peuvent pas — en le
consommant de l'extérieur, comme n'importe qui le ferait.

Le parti pris est celui des jeux de labyrinthe de la fin des années quatre-vingt-dix :
vue subjective, couloirs sombres, un plan de ce qu'on a déjà parcouru sur une
touche. Le labyrinthe est engendré, jamais dessiné à la main — une graine, et le
même se rejoue.

MIT ou Apache-2.0, au choix — voir [`LICENSE-MIT`](LICENSE-MIT) et
[`LICENSE-APACHE`](LICENSE-APACHE). Sauf mention contraire de son auteur, toute
contribution proposée à l'inclusion est placée sous ces deux mêmes licences. Les
planches, textures et sons sont produits pour le projet et suivent les mêmes
licences : **rien ici ne vient d'un jeu existant.** La seule exception est une
police en CC0, dont [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES) rend compte.

## État

**Étape 1 : le labyrinthe.** Un labyrinthe engendré qu'on parcourt à la première
personne, des étages reliés par des cages — à marches, ou en rampe. Pas encore un
jeu : ni monstres, ni tir, ni ramassages.

- [`ROADMAP.md`](ROADMAP.md) — les étapes et ce qui est hors périmètre
- [`CHANGELOG.md`](CHANGELOG.md) — ce que chaque version a apporté, daté
- [`docs/conception.md`](docs/conception.md) — ce que le jeu fait, et à qui chaque
  point le demande : au moteur, ou au jeu
- [`docs/construction.md`](docs/construction.md) — comment ça se construit, et
  comment la dépendance au moteur est épinglée

## Ce que le moteur ne fait pas, et que le jeu fait donc lui-même

Cette liste n'est pas une plainte : c'est la frontière, et elle a des raisons.

- **La glissade le long d'un mur.** Le moteur rend un temps d'impact et une
  normale, jamais une réponse. Glissade, marche, gravité sont des politiques de jeu.
- **Le tir contre un monstre.** Le moteur arrête un rayon sur la géométrie d'une
  cellule ; un monstre n'a pas de portail, donc pas d'adjacence. Décider qu'il est
  touchable est une règle de jeu.
- **La barre de vie, le score, l'arme en main.** Tout ce qui est en coordonnées
  d'écran se dessine dans le tampon après la fin d'image. Le moteur ne connaît pas
  d'interface, et son tracé de lignes prend des coordonnées de monde.
- **Le son.** Le moteur n'a ni horloge ni sortie audio, et n'en aura pas.

## Construction

```
make build    compile
make run      lance le jeu
make test     les tests
```

Le détail, et pourquoi la chaîne écrit où elle écrit :
[`docs/construction.md`](docs/construction.md).

## Commandes

Clic gauche prend le curseur, clic du milieu le rend, `Échap` quitte.

| Touche | Effet |
|---|---|
| `Z` `S`, ou flèches haut et bas | avancer, reculer |
| `Q` `D`, ou flèches gauche et droite | pivoter sur place |
| souris | orienter la vue, le curseur pris |

**On marche, on longe et on tombe** : les murs, le sol et le plafond retiennent, un pas
de biais suit la paroi au lieu de s'y coller, et la pesanteur ramène au sol.

**Les étages se parcourent** : une marche se monte sans sauter, une rampe se gravit
sans se redescendre, et un mur reste un mur. Il manque encore le pas de côté — les
touches latérales font pivoter.
