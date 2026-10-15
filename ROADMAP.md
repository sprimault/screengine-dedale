# Feuille de route

Les numéros font foi : ce sont eux que portent les `todo!("étape N : …")` du code.

```
rg -o 'todo!\("étape' src | wc -l
```

**Ce compte ne se lit jamais seul.** Il mesure ce qui a été *amorcé* puis laissé
en plan, et il compte zéro pour ce qu'une étape a écrit en prose et jamais
commencé. Zéro veut dire « rien d'entamé qui ne soit fini », jamais « rien ne
reste à faire » : ce qui reste est écrit en prose ci-dessous, et c'est l'écart
entre les deux qui informe.

**Les numéros ordonnent les dépendances, pas le calendrier.** Ils ne se
renumérotent jamais. Une étape qui apparaît s'ajoute à la fin, quelle que soit sa
place logique.

**Le jeu est jouable à la fin de l'étape 7.** Ce qui suit — l'ambiance, la vue de
dessus, le son — n'est pas de la finition : c'est ce que ce jeu existe pour
montrer du moteur. Mais une boucle de jeu d'abord, parce qu'on ne règle pas une
ambiance dans un décor qu'on ne parcourt pas.

**Le moteur ne bouge pas pour nous.** Chaque étape dit ce qu'elle demande au
moteur, et la réponse est toujours « rien de neuf » : la version épinglée porte
déjà tout ce qui suit. Si une étape devait réclamer une fonction, c'est elle qui
attend, pas le moteur qui cède.

---

## 0 — Le squelette

La fenêtre s'ouvre, la boucle tourne, le moteur est lié par un commit épinglé, et
une jauge se dessine dans le tampon de sortie. Un panneau plein cadre prouve que
la soumission aboutit.

**Son seul objet est de prouver que la chaîne tient**, et que l'interface a son
chemin dès le premier jour. Tout y est remplacé par la suite.

## 1 — Le labyrinthe

La grille engendrée, puis **exportée en cellules et portails** que le moteur charge
tel quel. L'algorithme de génération se choisit à cette étape ; il n'est pas ce
qu'elle a de difficile. Parcours à la première
personne, cellule de la caméra suivie par les traversées de portails.

**L'export est le vrai sujet**, pas la génération. Une cellule est un volume fermé
dont le sens de parcours décide de la face vue, ses portails sont appariés au bit
près des deux côtés, et le repère de lightmap a quatre contraintes :
`docs/cartes.md` de `screengine` les écrit, et rien ici ne les devine.

**Une graine, et le même labyrinthe se rejoue.** C'est ce qui rend tout le reste
reproductible — un monstre mal placé, un portail qui ne s'apparie pas, une
divergence de lightmap.

**Des étages superposés dès cette étape.** Le monde est en cellules 3D, pas en
plan : une passerelle qui enjambe un couloir déjà parcouru est ce qui distingue ce
modèle d'un labyrinthe à deux dimensions, et une grille plate n'en donnerait
jamais l'occasion.

## 2 — Le joueur

Déplacement filtré par le balayage d'une boîte contre les cellules, hauteur d'œil
déduite du centre du corps, glissade le long d'un mur.

**La glissade s'écrit ici**, en trois lignes de projection du déplacement sur la
normale du contact. Le moteur rend un temps d'impact et une normale, jamais une
réponse : un personnage arrêté net contre un mur en biais n'est pas un défaut du
moteur.

**Le volume décrit un corps, pas une tête.** Centré sur l'œil, il flotterait
au-dessus de ce qui est bas et le traverserait — géométriquement juste, et
parfaitement faux.

**L'arme en main se pose ici, et non à l'étape 5.** Deux cotes se décident l'une
contre l'autre : la demi-largeur du corps doit dépasser la distance de l'arme,
sinon un mur la coupe en longeant une paroi. Et son balancement est un instrument
de mesure du déplacement — sa phase avance avec la distance **réellement
parcourue**, donc des mains qui balancent alors qu'on est bloqué dénoncent un
déplacement appliqué avant la collision.

**Ce n'est pas de l'interface**, et c'est ce qui l'autorise : un sprite du monde
orienté caméra, posé à quelques décimètres de l'œil, qui reçoit le brouillard et
les lumières comme le reste. Un quadrilatère plein cadre reste interdit.

## 3 — Les monstres

Les planches de vues de `assets/`, choisies selon l'angle sous lequel on regarde la
créature, en quadrilatères orientés caméra. Déplacement par le balayage, tache
d'ombre modulée au sol.

**Trois silhouettes qui ne se confondent pas de loin** : c'est le critère qui a
décidé des planches, et il tient encore ici.

**Un sprite ne porte pas de normale** : un quadrilatère orienté caméra n'en a pas
et n'en prend pas. Un démon garde l'atténuation par la distance seule, et ce n'est
pas un manque à combler.

## 4 — Le tir et le contact

**Deux temps, et c'est la répartition que ce jeu existe pour montrer.** Le rayon
contre le décor est l'affaire du moteur : il rend le point d'impact et la surface
touchée, de quoi poser une marque sur un mur. Le test contre un monstre est
l'affaire du jeu, qui a placé son volume — décider qu'un démon est touchable est
une règle de jeu.

**Comment un démon montre qu'il est touché**, sans quoi tirer et rater se
ressemblent : un recul de deux ou trois images, décidé par le jeu, et un impact
posé au point de contact. Les deux se compensent, donc ils se décident ensemble.

**Le geste existe déjà**, posé à l'étape 2 avec l'arme : le clic droit montre la
pose de tir le temps d'un éclair, sans aucun effet. Il ne reste donc ici qu'à y
accrocher ses conséquences — le rayon, la marque, le test de volume.

## 5 — La vie, le score, l'interface

Tout dans le tampon de sortie, par `run_with_output` : barre de vie, score, texte.
**Rien ne passe par le moteur** — ni un quadrilatère plein cadre, ni le tracé de
lignes, qui prend des coordonnées de monde.

L'arme en main n'en fait pas partie et s'est posée à l'étape 2 : elle est un objet
du monde, pas de l'écran.

Contact avec un monstre, perte de vie, mort du joueur.

## 6 — Les ramassages

Munitions et fioles de soin, posées par le jeu et testées par lui contre la pose
du joueur. Le décor ne les connaît pas.

## 7 — La sortie et l'enchaînement

Trouver la sortie, enchaîner un labyrinthe suivant, écran de fin et de score.
**À ce point, c'est un jeu.**

**Une seule carte et une seule traversée** par labyrinthe : enchaîner, c'est
charger la carte suivante à côté, basculer, détruire l'ancienne. Le moteur n'a
aucune fonction pour cela, et c'est son parti — `ScgWorld` reste immuable.

## 8 — L'ambiance

**Un éclairage général bas, qui ne montre pas tout, et des tubes au plafond dont la
lumière tremble.** Lightmaps cuites tamisées, lumières dynamiques pour les tubes,
brouillard par la distance, résolution interne basse remontée en entier.

**C'est le sujet d'éclairage le plus intéressant du jeu**, et la raison pour
laquelle il arrive après la boucle : on ne règle pas un tamisage dans un décor
qu'on ne parcourt pas.

**Plus de huit lumières dynamiques n'existe pas** : c'est une constante du moteur.
Tout ce qui dépasse se cuit ou se module.

**C'est ici que l'image animée du `README` se produit**, et pas avant : plus tôt elle
montrerait un couloir sans créature et sans lumière, ce qui dessert le jeu comme le
moteur. En WebP animé, comme celle du moteur, et par la même chaîne.

Ce qu'elle demande en plus : **un chemin de rendu hors fenêtre.** La boucle de
`screengine-play` ouvre une fenêtre, donc elle ne sert pas ; il faut appeler le
moteur directement sur la scène du jeu, par où l'outillage de la chaîne passe déjà
pour le moteur. La scène à rendre doit donc être construite par une fonction que la
boucle et ce chemin appellent tous deux — à prévoir en écrivant les étapes
précédentes, pas à démêler ici.

## 9 — La vue de dessus

Le plan du labyrinthe sur une touche, tracé en lignes et en points, en mode
visible à travers. La progression se révèle à mesure.

**Rien à ajouter au moteur pour cela** : son tracé prend des segments en
coordonnées de monde, et le mode visible à travers est exactement ce qu'un plan
demande. C'est aussi ce qui rend la progression lisible — on voit ce qu'on a
parcouru, pas ce qui reste.

## 10 — Le son

Un cri par créature, l'ambiance, la musique. **Rien du moteur** : il n'a ni
horloge ni sortie audio, et n'en aura pas. Tout vit ici.

**Le son arrive avant la vue** : c'est ce qui donne à un monstre hors champ sa
présence, et c'est pour cela que les trois silhouettes ont été choisies pour ne pas
se confondre — on les entend avant de les distinguer.

## 11 — Les pièces

Le générateur tire des rectangles de cases et ouvre tous leurs murs intérieurs,
puis perce le labyrinthe autour comme avant. Quatre ou cinq par étage.

**L'export ne bouge pas, et c'est ce qui rend l'étape bon marché.** L'empreinte
d'un passage s'étend de `inner_low` à `inner_high` sur tout le côté — pleine
largeur, pas une embrasure. Deux cases ouvertes l'une sur l'autre donnent donc
3,0 de cellule, 1,0 de passage pleine largeur, 3,0 de cellule : un volume continu
sans seuil ni rétrécissement. Un bloc de cases entièrement percé **est** déjà une
salle ouverte, que le format décrit sans rien y ajouter.

Les identifiants de surface restent fonction de la seule position, l'empreinte du
cache de lightmaps reste stable, et l'appariement au bit près tient. Seul `grid`
change.

**L'algorithme de l'étape 1 ne peut pas produire cela.** Un labyrinthe parfait est
un arbre couvrant : jamais un bloc de 2×2 entièrement ouvert, par construction.
C'est une passe de plus, avant le perçage, pas un réglage de l'existant.

**Ce que la pièce apporte, et qui n'était pas prévu : le combat existe.** Dans un
couloir de trois unités face à un démon qui en occupe la largeur, les pas de côté
ne servent à rien et le sprite remplit l'écran. Une salle donne de quoi tourner
autour, reculer, se replacer.

**Et elle est un repère.** Des couloirs tous identiques ne se mémorisent pas, et
se perdre sans rien reconnaître n'est pas excitant, c'est fatigant. Le joueur qui
reconnaît une salle se construit une carte mentale, et se sentir compétent dans un
lieu hostile est ce qui retient. Il n'y a donc rien à greffer pour obtenir des
repères : il suffit qu'il y ait des pièces.

**Ce que cela coûte : des cellules, pas des surfaces.** Une pièce de 3×3 cases
pèse neuf cellules de case plus douze passages, soit vingt et une au lieu de neuf.
Le contrat du moteur est formel — le chargement est linéaire en nombre de cellules,
et c'est l'axe des surfaces d'une même cellule qui est super-linéaire. Celui-là ne
bouge pas : chaque prisme en garde six.

**Ce que cela écarte.** Le labyrinthe parfait avait une propriété qui disparaît :
une seule solution, et la main sur le mur droit finit toujours par sortir. Les
pièces créent des boucles. Pour un jeu c'est mieux — une boucle est une
échappatoire, et c'est ce qui rend une poursuite jouable — mais la connexité ne va
plus de soi et le générateur doit la vérifier au lieu de l'obtenir par
construction.

**Deux choses à surveiller, qui ne se verront qu'à l'étape 8.** Le sol d'une pièce
n'est pas une surface mais une mosaïque de sols de cases et de passages : en
couloir les raccords de lightmap tombent là où on ne regarde pas, dans une grande
salle éclairée en biais ils sont en plein champ. Et la portée d'une lampe de pièce
couvre une case et déborde d'un portail — neuf lampes dans une pièce de 3×3
feraient une nappe uniforme et sans intérêt. C'est au générateur de décider où en
poser une, et une pièce éclairée d'un seul côté se retient mieux qu'une pièce
éclairée partout.

Demande au moteur : rien. Le générateur suffit.

## 12 — La descente

Deux étages par labyrinthe, trois au plus, et la sortie est une cage qui descend
vers le labyrinthe suivant.

**Les étages restent, et c'est l'étape 1 qui l'exige.** Une passerelle qui enjambe
un couloir déjà parcouru est ce qui distingue des cellules 3D d'une grille plate ;
les retirer renoncerait à la seule chose que ce jeu démontre du moteur en matière
de géométrie, et rendrait le plan de l'étape 9 trivial.

**Mais moins d'étages par labyrinthe, et plus de labyrinthes.** Avec des pièces, un
labyrinthe de quatre étages est long à charger, long à cuire, et on s'y perd mal
plutôt que bien. La verticalité doit rester une surprise de parcours, pas une
quantité.

**La sortie prend la forme d'une cage.** Elle existe déjà, elle relie deux étages,
et la réemployer pour descendre d'un labyrinthe au suivant donne une direction à la
chaîne : on descend, on ne passe pas à autre chose. L'enchaînement cesse d'être un
écran de chargement.

**Elle se distingue des cages d'étage**, par son éclairage ou son thème. Un joueur
qui s'engage dans une descente abandonne ce qu'il n'a pas fouillé : la décision
doit être informée, sinon elle n'en est pas une et le jeu paraît déloyal.

**Le mécanisme de l'étape 7 ne change pas** : charger la carte suivante à côté,
basculer, détruire l'ancienne. Ce qui change est ce que le joueur en voit.

Demande au moteur : rien de neuf. La cage est celle de l'étape 1, l'enchaînement
celui de l'étape 7. Dépend de 1 et de 7.

## 13 — La pression du temps

Le labyrinthe se peuple à mesure qu'on y reste : un démon de plus à intervalle
régulier, entrant par une cellule éloignée de celle du joueur.

**Sans cela, l'arbitrage de l'étape 14 est faux.** Si rester ne coûte rien, on
fouille tout, toujours, et il n'y a plus de décision à prendre — seulement une
procédure à dérouler. La pression n'existe pas pour durcir le jeu, elle existe pour
que renoncer soit parfois le bon choix.

**Ce n'est pas une difficulté qui monte en chiffres.** Aucun démon ne gagne de vie
ni de dégâts avec le temps ; il en arrive simplement d'autres. Un couloir qu'on
croyait nettoyé et qui ne l'est plus fait plus d'effet qu'un démon deux fois plus
résistant.

**L'arrivée se voit avant de se subir** : le démon entre loin, et l'étape 10 le fait
entendre. Une apparition dans le dos du joueur serait une punition arbitraire, pas
une pression.

Demande au moteur : rien. Placement et compte sont au jeu, comme à l'étape 3.
Dépend de 7.

## 14 — Les caches et les trésors

Des cellules que le labyrinthe ne dessert pas, fermées par une fausse paroi, et ce
qu'on y trouve.

**Une cache est une vraie cellule avec un vrai portail**, et ce qui la ferme est un
maillage soumis devant lui — pas de la géométrie de carte. Le monde reste immuable,
rien ne se recharge, et l'ouvrir consiste à cesser de soumettre ce maillage.
L'alternative, un mur qui coulisse dans la géométrie, exigerait de remplacer la
carte entière : l'étape 7 a tranché que cela n'arrive qu'entre deux labyrinthes.

**La fausse paroi n'occulte ni la lumière ni le rayon, et c'est voulu.** Les deux
conséquences sont les deux indices, et elles ne coûtent pas une ligne :

- les lightmaps se cuisent sur la carte, que le maillage n'encombre pas, donc une
  source faible posée dans la cache répand une tache tiède au pied d'une paroi qui
  ne devrait rien éclairer ;
- le rayon du moteur ne voit que le décor, donc il traverse le portail et frappe le
  fond de la cache, et la marque se pose là où aucun mur ne se voit.

Quelqu'un lira cela comme un défaut dans six mois. Sans ce paragraphe, il le
corrigera et supprimera les deux indices d'un coup.

**Une cache se lit mieux dans une pièce que dans un couloir.** Dans un couloir, une
fausse paroi est l'une des deux cents parois identiques. Dans une salle, c'est un
mur dont l'épaisseur ne correspond pas à ce qu'on a vu de l'autre côté — le joueur
raisonne au lieu de sonder au hasard. C'est la dépendance à l'étape 11, et elle est
de fond.

**Tirer sur une paroi coûte une munition**, la même qu'il faudra au démon du couloir
suivant. C'est le lien qui fait tenir l'étape : chercher devient une dépense et non
une habitude, et entre celui qui sonde tout et celui qui ne sonde jamais il y a un
jugement à exercer. Cela suppose que les munitions soient rares — c'est la seule
chose que cette étape demande à l'étape 6.

**Ce qu'on trouve change quelque chose, et ce ne sont pas des chiffres** : une lampe
qui porte plus loin, une boussole qui donne la direction de la sortie, une marque
qui reste sur le plan de l'étape 9, une capacité de munitions. Trois ou quatre en
tout. Un trésor qui ne ferait que monter le score ne ferait chercher personne : on
en gagne déjà, et plus sûrement, en abattant des démons.

**La sortie est toujours trouvable, les caches sont facultatives.** S'y engager clôt
le labyrinthe et abandonne ce qu'on n'a pas fouillé. C'est la seule décision que ce
jeu demande, et elle se répète à chaque salle.

Demande au moteur : rien de neuf. Le maillage passe par la soumission qui existe
depuis l'étape 0, le test de sa boîte est au jeu comme celui d'un démon à l'étape 4,
et la cuisson est celle de l'étape 8. Dépend de 4 pour le rayon, de 6 pour la
rareté, de 8 pour l'éclairage tamisé sans lequel la fuite de lumière n'est pas un
indice, et de 11 pour que la cache soit lisible.

## 15 — Les thèmes

Un thème est un jeu de textures, un réglage d'éclairage cuit et une densité de
démons, choisis par le rang du labyrinthe dans la descente.

**C'est un paramètre de génération, pas un mécanisme.** L'enchaînement ne change
pas, et la cage de descente porte le thème de ce vers quoi elle mène — ce qui est
aussi ce qui la distingue d'une cage d'étage.

**Le coût n'est pas dans le code, il est dans les planches.** Chaque thème est un
jeu de textures à produire, et c'est ce qui décidera de leur nombre. Deux thèmes
bien distincts valent mieux que cinq qui se ressemblent.

Demande au moteur : rien de neuf. Dépend de 7 et de 12.

## 16 — Ce qui se garde d'une descente à l'autre

Les capacités trouvées à l'étape 14 passent au labyrinthe suivant. Rien d'autre ne
se garde.

**Pas de statistiques, pas d'arbre, pas d'écran d'amélioration.** Ce qui retient
n'est pas la progression du personnage, c'est celle du joueur : reconnaître une
fuite de lumière, savoir qu'une salle trop régulière cache quelque chose. Les trois
ou quatre capacités sont là pour que la fouille ait une conséquence durable, pas
pour construire un système.

**Mourir coûte des secondes, pas des minutes.** La touche de relance existe depuis
l'étape 5 ; ce qui s'ajoute ici est l'interdiction de tout ce qui s'intercalerait —
écran de score à valider, choix à faire au calme. Ce qui termine une session n'est
pas l'échec, c'est le temps mort qui le suit.

Dépend de 14 et de 15.

---

## Hors périmètre

- **La collision contre un volume mobile par le moteur.** Un monstre n'a pas de
  portail donc pas d'adjacence : rien de ce que le balayage traverse ne s'applique
  à lui. Le test est ici, et ce n'est pas un report.
- **La transparence graduée** — fumée, halo, verre : la transparence du moteur est
  binaire, et un mélange fractionnaire y ramènerait le tri par profondeur que le
  z-buffer a supprimé. Ce qui s'en approche sans le coût est la surface modulée.
- **Des ennemis qui poursuivent hors de vue, d'une cellule à l'autre** : cela
  suppose une navigation, donc une structure que le jeu doit tenir lui-même. La
  carte donne les portails, pas un graphe de déplacement. **À reconsidérer après
  l'étape 13**, qui fera entrer des démons pendant la partie : les pièces de
  l'étape 11 rendent la question plus pressante — une salle est l'endroit où une
  poursuite se jouerait — et les boucles qu'elles créent donnent au joueur
  l'échappatoire qui la rend tenable.

  **La poursuite à vue, elle, est au périmètre**, et c'est la distinction qui
  décide : une ligne de vue dégagée **est** un chemin, que le rayon du moteur rend
  sans qu'aucun graphe n'existe. Une créature qui voit le joueur se dirige vers
  lui ; c'est le moment où elle le perd de vue qui demande ce que la carte ne donne
  pas.
- **Une visibilité précalculée** pour accélérer un grand labyrinthe : le moteur la
  refuse par principe, et sa traversée est faite pour s'en passer.
- **Le multijoueur**, sous toutes ses formes.
- **Des statistiques chiffrées et un arbre de capacités.** L'étape 16 garde trois
  ou quatre capacités, et c'est une limite, pas un début. Un système de progression
  déplacerait le jeu vers ce que l'interface sait faire, et ce jeu n'a pas
  d'interface.
- **Une sauvegarde de partie en cours.** Une descente se joue d'un trait ; une
  reprise supposerait de restituer l'état des démons, des caches ouvertes et du
  peuplement de l'étape 13, pour un gain nul sur une partie de quelques minutes.
- **Un menu entre la mort et la partie suivante.** Conséquence directe de
  l'étape 16, répétée ici parce que c'est le genre d'ajout qui paraît anodin au
  moment où on le fait.
