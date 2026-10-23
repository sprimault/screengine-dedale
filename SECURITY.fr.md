# Sécurité

English: [SECURITY.md](SECURITY.md)

## Versions suivies

La dernière version publiée. Il n'y a pas de branche de maintenance : un
correctif sort dans la version suivante, jamais en rétroportage sur une version
antérieure.

## Signaler une faille

Par un avis de sécurité privé sur le dépôt GitHub, jamais par une issue
publique. Réponse sous quelques jours.

## Ce qui est dans le périmètre

**Le jeu ne lit rien qu'il n'ait écrit lui-même, et c'est ce qui dessine son
périmètre.** L'exécutable est autonome : les planches, les textures et la police
entrent dedans à la compilation, le labyrinthe est engendré depuis une graine, et
rien n'est cherché au lancement — ni fichier de configuration, ni argument de
ligne de commande, ni variable d'environnement. Il n'existe donc aucune entrée
par laquelle un contenu hostile arriverait, et c'est volontaire.

Ce qui reste, et qui compte :

- une archive ou un programme d'installation attaché à une release dont le
  contenu ne correspond pas à ce que le dépôt porte au tag ;
- un avis de sécurité qui touche une dépendance du jeu, si les contrôles du
  dépôt ne l'ont pas déjà levé ;
- un défaut du jeu qui lirait, écrirait ou exécuterait quelque chose en dehors de
  ce qu'il annonce faire, ou qui donnerait à un contenu chargé le moyen de
  s'exécuter — **rien de ce qui entre dans le binaire ne le permet** : ce sont
  des blocs de texels et une police, que le jeu copie sans jamais les
  interpréter.

## Ce qui n'y est pas

**Le moteur de rendu a son propre canal.** `screengine` est une dépendance
épinglée par commit, et c'est lui qui décode des octets : une faille dans son
chargement de textures, de maillages ou de cartes se signale dans son dépôt, qui
porte le périmètre correspondant. Ne relève d'ici que ce que le jeu lui remet.

**Les archives ne sont pas signées**, ni sur Windows ni ailleurs, et
`docs/construction.md` le dit : un exécutable téléchargé déclenche donc un
avertissement du système. C'est une limite connue et assumée, pas une faille.

Modifier ses propres fichiers pour obtenir un jeu différent. Rien ici n'est à
protéger contre son propriétaire.
