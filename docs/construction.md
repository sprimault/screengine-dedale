# Construction

Comment le jeu se construit, où va quoi, et comment la dépendance au moteur est
épinglée.

## Passer par le `Makefile`

Toute construction passe par `make`, jamais par `cargo` en direct. Le `Makefile`
inclut `makefile.local`, qui porte ce qui est propre à un poste : répertoire de
sortie, chaîne d'outils, variables temporaires. Une commande tapée à la main perd
ces réglages, et l'écart ne se voit pas dans la sortie.

`makefile.local` n'est pas versionné. Un clone sans lui compile dans `target/` à la
racine, ce qui marche : la redirection est une contrainte de poste, pas du projet.

| Cible | Ce qu'elle fait |
|---|---|
| `make build` | compile, profil dev — les dépendances y sont optimisées, sans quoi le rendu logiciel est injouable |
| `make run` | lance le jeu dans une fenêtre |
| `make release` | compile en release, LTO et une unité de génération |
| `make test` | les tests |
| `make attentes` | les épreuves écrites avant le correctif du moteur qu'elles attendent, que `make test` saute |
| `make fmt` | `cargo fmt --check` |
| `make lint` | clippy en `-D warnings`, cibles de test comprises |
| `make deny` | licences, sources, doublons |
| `make audit` | les avis de sécurité, interrogés **en direct** |

Avant de pousser, la liste entière :

```bash
make fmt && make lint && make test && make deny && make audit
```

**Et `make run`, à l'œil, dès que le changement se voit.** Un jeu ouvre une
fenêtre : rien ne l'éprouve automatiquement, et aucun test ne dira qu'une jauge est
illisible ou qu'un démon apparaît dans un mur.

## La dépendance au moteur

`Cargo.toml` épingle `screengine-play` par **version et par commit** :

```toml
screengine-play = { version = "0.9", git = "…/screengine", rev = "…" }
```

**Par git et non par chemin** : un chemin ne vaudrait que sur un poste où les deux
dépôts sont côte à côte, et l'intégration continue le refuserait. Prendre le moteur
comme n'importe qui le prendrait est aussi ce que ce dépôt doit éprouver — que
l'API telle qu'elle sort suffit à un consommateur extérieur. C'est une des deux
raisons d'être de ce dépôt.

**Par commit et non par tag**, et c'est une conséquence de ce que les archives de
release du moteur emportent : la bibliothèque C et son header, jamais
`screengine-play`, qui est un paquet Rust. Un tag n'a donc pas de destinataire ici,
et un commit épingle aussi exactement. On passera au tag le jour où une version
publiée portera ce dont ce jeu dépend.

**La `version` en plus du commit n'est pas redondante** : sans elle la dépendance
est un joker, que `cargo deny` refuse — et à juste titre, puisque rien ne dirait
alors contre quelle version de l'API ce jeu est écrit.

### Aucune redirection locale

**Le moteur se récupère depuis son dépôt, partout et pour tout le monde**, y compris
sur le poste où les deux sources sont côte à côte. Un `[patch]` vers un clone voisin
ferait compiler contre une API que le commit épinglé n'a pas, et l'écart ne se
verrait qu'en intégration continue.

Il coûtait en outre un verrou faux : avec un patch par chemin, `cargo` réécrit
`Cargo.lock` **sans la ligne `source`** de la dépendance. Le verrou commité décrivait
donc un montage qu'un seul poste avait.

Le besoin qu'il servait — essayer une API du moteur avant de la pousser — n'en est
pas un ici : un manque se décrit, part dans l'autre dépôt, et le jeu attend ou
contourne.

## Où vont les artefacts

Sur le poste de développement sous Windows, **l'antivirus met en quarantaine les
exécutables au moment où le linker les produit** : la compilation échoue sur un
accès refusé, sans rapport avec le code et de façon intermittente. Tout ce que la
chaîne Rust écrit va donc dans `.tmp/`, qui est en exception de l'antivirus.

Deux variables ne suffisent pas :

- `CARGO_TARGET_DIR` gouverne la compilation, et il pointe sur le `.tmp/` **de ce
  dépôt** ;
- `TMP` et `TEMP` sont posés aussi, parce que **rustdoc n'honore pas
  `CARGO_TARGET_DIR` pour les doctests** : il compile dans un répertoire tiré de
  `env::temp_dir()`, et le binaire qu'il y produit s'appelle toujours
  `rust_out.exe`. Un exécutable neuf hors du dépôt est un fichier inconnu de plus à
  chaque `make test`.

**La chaîne d'outils est propre à ce dépôt**, dans son `.tmp/` : une seule chaîne,
stable, et la seule cible hôte. Elle a d'abord été empruntée à celle du moteur, et
le jeu cessait de compiler dès que le clone voisin bougeait. C'est la même raison
qui a emporté la redirection de dépendance : prendre le moteur comme n'importe quel
consommateur extérieur, qui n'a pas ce clone sous la main, est une des deux raisons
d'être de ce dépôt — et rien de ce qui vit à côté ne doit pouvoir le faire mentir.

## Les assets

`assets/` porte les sorties **validées** de la chaîne, rangées par type :
`sprites/`, `textures/`, `skies/`, `sounds/`, `music/`, `fonts/`, `icons/`. Elles
sont versionnées : le jeu ne tourne pas sans elles.

**`icons/` est le seul répertoire dont le jeu ne lit rien à l'exécution** : son
icône entre dans l'exécutable à la compilation, par une ressource Win32 que
`build.rs` pose, et les installateurs la reprennent depuis le dépôt.

La chaîne qui les produit vit **hors du dépôt** : des modèles, du rendu hors écran et
de l'outillage, dont le dépôt n'a besoin que des sorties.

`.gitattributes` déclare les formats binaires plutôt que de s'en remettre à
l'heuristique de `text=auto` : une planche convertie en fins de ligne ne se voit
qu'au moment où elle ne se décode plus.

**Rien de ce qui entre ici ne vient d'un jeu existant.** Textures, palettes,
colormaps, cartes, maillages et sons extraits de jeux commerciaux ne sont pas
redistribuables, et c'est la tentation naturelle d'un jeu de cette famille.

## Ce qui se distribue

**Une archive par système, attachée à la release.** Le jeu ne se publie pas sur
`crates.io` — c'est une application, pas une bibliothèque —, donc ce qu'on en prend
est un exécutable.

**Et cet exécutable est autonome.** Les planches, les textures et la police entrent
dedans par `include_bytes!` : il n'y a rien à poser à côté de lui, rien à chercher au
lancement, et aucun répertoire d'assets à distribuer. L'archive ne porte le `README`,
le `CHANGELOG` et les licences que parce qu'une archive sans eux ne dit pas ce qu'elle
contient.

**Le binaire Linux se construit sur la plus ancienne image encore offerte**, jamais sur
la plus récente : il se lie dynamiquement à la glibc, donc un exécutable construit sur
une distribution récente refuse de démarrer sur une plus ancienne — et le défaut
n'apparaît que chez celui qui télécharge. C'est aussi pourquoi le libellé mobile
`ubuntu-latest` est écarté : il change de version sans que ce dépôt bouge, exactement
comme un tag d'action mobile change de contenu.

**Rien n'est signé**, ni sur Windows ni ailleurs : un exécutable téléchargé déclenche
donc un avertissement du système, qu'un installateur non signé ne lèverait pas
davantage.

**Deux formes d'installation s'ajoutent aux archives**, décrites dans `packaging/` :
un programme d'installation Windows, et une AppImage pour Linux. Le binaire étant
autonome, ni l'un ni l'autre ne copie quoi que ce soit d'essentiel — leur intérêt est
dans ce que le système en sait : un raccourci, une désinstallation propre, une entrée
dans la liste des programmes.

**L'AppImage est la seule des deux qui corrige quelque chose** : son fichier
`.desktop` donne au jeu son icône sous X11 et Wayland, que rien d'autre ne peut poser.

**L'icône de l'exécutable se pose à la compilation, et elle ne vaut que pour
Windows.** C'est une ressource du binaire, donc l'explorateur, la barre des tâches
et la fenêtre la montrent toutes les trois — une fenêtre sans icône explicite prend
celle de son exécutable. Sous X11 et Wayland il n'y a pas d'équivalent : l'icône y
vient du fichier `.desktop` posé à l'installation, et un binaire lancé à la main
garde celle du système.
