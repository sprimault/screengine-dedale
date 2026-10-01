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
screengine-play = { version = "0.8", git = "…/screengine", rev = "…" }
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

### Itérer sur les deux à la fois

`.cargo/config.toml`, local et hors dépôt, redirige la dépendance vers le clone
voisin :

```toml
[patch."https://github.com/sprimault/screengine"]
screengine-play = { path = "../screengine/crates/screengine-play" }
```

**Le `Cargo.toml` reste la vérité** : le retirer, et la compilation repart sur le
commit épinglé, ce qui est le bon comportement.

**Le piège propre à ce montage** : une API ajoutée au clone voisin et pas encore
poussée fait compiler ici et échouer en intégration continue. C'est voulu — c'est ce
qui rappelle d'avancer le `rev` avant de livrer, dans le même commit que le code qui
en a besoin.

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
ce n'était pas tenable — la redirection de `.cargo/config.toml` est optionnelle et
retombe sur le commit épinglé, alors qu'une chaîne empruntée ne retombe sur rien :
le jeu cessait de compiler dès que le clone voisin bougeait. Or prendre le moteur
comme n'importe quel consommateur extérieur, qui n'a pas ce clone sous la main, est
une des deux raisons d'être de ce dépôt.

## Les assets

`assets/` porte les sorties **validées** de la chaîne, rangées par type :
`sprites/`, `textures/`, `skies/`, `sounds/`, `music/`, `fonts/`. Elles sont
versionnées : le jeu ne tourne pas sans elles.

La chaîne qui les produit vit dans `fabrique/`, **hors dépôt** : des modèles, du
rendu hors écran et de l'outillage, dont le dépôt n'a besoin que des sorties.

`.gitattributes` déclare les formats binaires plutôt que de s'en remettre à
l'heuristique de `text=auto` : une planche convertie en fins de ligne ne se voit
qu'au moment où elle ne se décode plus.

**Rien de ce qui entre ici ne vient d'un jeu existant.** Textures, palettes,
colormaps, cartes, maillages et sons extraits de jeux commerciaux ne sont pas
redistribuables, et c'est la tentation naturelle d'un jeu de cette famille.
