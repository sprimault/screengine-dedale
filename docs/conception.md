# Conception

Ce que le jeu fait, et **à qui chaque point le demande** : au moteur, ou au jeu.

Ce document existe pour une raison précise : la frontière entre les deux n'est pas
intuitive. Trois besoins sur quatre qui semblent exiger une fonction du moteur sont
des règles de jeu, et la table ci-dessous est ce qu'on relit avant de réclamer
quoi que ce soit à l'autre dépôt.

Le contrat du moteur fait foi ailleurs : `docs/abi.md` de `screengine`. On ne le
recopie pas ici.

## L'idée

Sortir d'un labyrinthe. On y ramasse des munitions et des fioles de soin, des
monstres le parcourent, il faut les abattre ou perdre de la vie à leur contact.
Barre de vie, score, ambiance sombre.

Le parti pris est celui des jeux de labyrinthe de la fin des années
quatre-vingt-dix : vue subjective, couloirs sombres, un plan de ce qu'on a déjà
parcouru sur une touche. Le labyrinthe est **engendré, jamais dessiné à la main** —
une graine, et le même se rejoue.

## Qui fait quoi

| Ce qu'on veut | Où ça se fait |
|---|---|
| Labyrinthe en cellules et portails | le jeu l'engendre et l'écrit ; le moteur le charge tel quel |
| Rejouer le même labyrinthe | une graine, côté jeu |
| Enchaîner des labyrinthes | charger la carte suivante, basculer, détruire l'ancienne |
| Parcours à la première personne | `submit_world_visible` + suivi de cellule |
| Se rallumer sans recuire | le cache de lightmaps du moteur |
| Déplacement arrêté par le décor | le balayage du moteur rend un temps d'impact et une normale |
| **Glissade le long d'un mur** | **le jeu** — trois lignes de projection sur la normale |
| Sprites d'ennemis, orientés caméra | `submit_sprites` |
| Tache d'ombre sous un ennemi | surface modulée |
| Tir **contre le décor** | le rayon du moteur, qui rend la surface touchée |
| **Tir et contact contre un monstre** | **le jeu**, qui a placé son volume |
| **Ramassage** | **le jeu**, contre les poses qu'il place |
| Déplacement des monstres | le balayage du moteur + la logique du jeu |
| **Barre de vie, score, arme en main** | **le jeu**, dans le tampon de sortie |
| Vue de dessus | le tracé de lignes du moteur, en mode visible à travers |
| Ambiance sombre | lightmaps cuites tamisées, brouillard, tubes modulés |
| **Son** | **le jeu** — le moteur n'a ni horloge ni sortie audio |
| **Navigation d'un monstre d'une cellule à l'autre** | **le jeu** — la carte donne les portails, pas un graphe |

Les lignes en gras sont celles où l'on se trompe. Les autres sont déjà au moteur.

## Les quatre frontières du moteur

Ce ne sont pas des trous à combler, mais des bords qui ont une raison.

**Il n'arrête que la géométrie de cellule.** Un monstre, un objet posé, une caisse
n'ont pas de portail, donc pas d'adjacence : rien de ce que le balayage traverse ne
s'applique à eux. Un volume mobile encore moins — décider qu'un démon est touchable
est une règle de jeu.

**Il ne rend aucune réponse de collision**, seulement un temps d'impact et une
normale. Glissade, marche, gravité, rebond sont des politiques du jeu.

**Sa transparence est binaire.** Pas de fumée, pas de halo, pas de verre en
transparence graduée : un mélange fractionnaire ramènerait le tri par profondeur que
le z-buffer a supprimé. Ce qui s'en approche sans le coût est la surface modulée.

**Il ne connaît rien du jeu** : ni joueur, ni arme, ni ennemi, ni santé, ni score.
Si une fonction de rendu semblait réclamer une de ces notions, c'est qu'il lui
manque un paramètre générique — jamais qu'il faut une exception.

## Ce que le jeu doit montrer du moteur

Une démonstration qui ne fait qu'être jouable ne démontre rien. Ce qui mérite
d'être exercé, parce que c'est ce que ce moteur a de particulier :

- **des étages superposés**, qu'un labyrinthe de grille ne donne pas de lui-même.
  Le monde est en cellules 3D, pas en plan : une passerelle qui enjambe un couloir
  déjà parcouru est ce qui distingue ce modèle d'un labyrinthe à deux dimensions ;
- **et des escaliers qu'on gravit pour y aller**, parce que c'est ce qui ouvre les
  décors d'intérieur : un jeu qui se passe dans un immeuble n'a pas le choix. Une
  cage est une cellule **concave** — le volume reste ouvert au-dessus des marches
  —, et son portail ne coupe qu'une tranche de son mur en hauteur ;
- **des rampes dans une partie de ces cages**, qui apportent les seules **surfaces
  obliques** du décor, donc les seuls repères de lightmap obliques. Les deux
  coexistent parce qu'elles ne montrent pas la même chose : échanger l'une contre
  l'autre retirerait une épreuve pour en gagner une ;
- **des tubes lumineux au plafond, qui scintillent**, dans un décor tamisé qui ne
  montre pas tout — c'est le sujet d'éclairage le plus intéressant du jeu ;
- **le brouillard par la distance**, qui ferme la vue au bout de quelques cellules :
  il sert l'ambiance et la performance par le même geste ;
- **la résolution interne basse remontée en entier**, qui donne le grain de l'époque
  visée et fait tourner le tout sur un téléphone ;
- **la tache d'ombre modulée** sous chaque monstre, qui assombrit les dalles au lieu
  de les recouvrir ;
- **l'édition à chaud** : la carte se remplace entière — on la charge à côté, on
  bascule, on détruit l'ancienne.

## Le labyrinthe mesure ce que rien ne mesurait

**Un décor à centaines de cellules dont on n'en voit que trois est ce qui manque au
moteur pour mesurer sa traversée.** Le gain relevé sur ses propres décors n'est pas
un verdict sur elle : c'en est un sur des décors où presque tout est visible.

C'est aussi ce décor qui éprouvera la borne de profondeur du balayage, qui vaut 64
sans avoir jamais été approchée.

À faire **avec le premier export**, sans attendre que le jeu soit jouable : la
mesure intéresse l'autre dépôt, et elle n'a pas besoin d'un joueur.
