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
`../screengine/docs/cartes.md` les écrit, et rien ici ne les devine.

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

## 5 — La vie, le score, l'interface

Tout dans le tampon de sortie, par `run_with_output` : barre de vie, score, arme
vue en main. **Rien ne passe par le moteur** — ni un quadrilatère plein cadre, ni
le tracé de lignes, qui prend des coordonnées de monde.

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

---

## Hors périmètre

- **La collision contre un volume mobile par le moteur.** Un monstre n'a pas de
  portail donc pas d'adjacence : rien de ce que le balayage traverse ne s'applique
  à lui. Le test est ici, et ce n'est pas un report.
- **La transparence graduée** — fumée, halo, verre : la transparence du moteur est
  binaire, et un mélange fractionnaire y ramènerait le tri par profondeur que le
  z-buffer a supprimé. Ce qui s'en approche sans le coût est la surface modulée.
- **Des ennemis qui se poursuivent d'une cellule à l'autre** : cela suppose une
  navigation, donc une structure que le jeu doit tenir lui-même. La carte donne les
  portails, pas un graphe de déplacement. À reconsidérer après l'étape 7.
- **Une visibilité précalculée** pour accélérer un grand labyrinthe : le moteur la
  refuse par principe, et sa traversée est faite pour s'en passer.
- **Le multijoueur**, sous toutes ses formes.
