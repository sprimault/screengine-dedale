// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La résolution interne que la fenêtre réclame — une politique du jeu.
//!
//! **Le moteur ne la choisit pour personne, et son contrat dit pourquoi** : il n'a ni
//! fenêtre, ni écran, ni horloge, et c'est le nombre de pixels qui décide du temps de
//! remplissage. Ce qu'un rendu logiciel tient est donc un arbitrage du jeu, pris une
//! fois ici plutôt que dispersé dans la boucle.
//!
//! **Aucune cote d'écran n'est écrite ici, et le plafond du contexte ne vient même plus
//! de ce module** : l'étage d'accueil le prend de l'enveloppe des écrans, bornée par la
//! borne du moteur, parce qu'eux seuls la connaissent — les écrans ne sont lisibles
//! qu'une fois la boucle lancée, donc après le moment où le plafond se règle. Ce qu'il
//! décide est la mémoire, jamais l'image.
//!
//! **Ce qui tient la cadence n'est donc pas le plafond mais le grain** : un facteur
//! d'agrandissement minimum, qui est le parti pris que `docs/conception.md` écrit — la
//! résolution interne basse remontée en entier. Un plafond employé comme budget
//! reviendrait à figer une cote, et le jeu perdrait son grain dès qu'une fenêtre
//! tiendrait sous elle.
//!
//! **Et ce que cela rend n'est pas du champ de vision**, ce qui est la méprise à éviter
//! en réglant : à aspect égal, le cadrage ne bouge pas d'un pixel, le champ vertical
//! étant constant. Ce qui se gagne est le nombre d'échantillons que l'atténuation
//! reçoit — un mur lointain tient sur quelques pixels larges à basse résolution, donc
//! son dégradé se quantifie en trois ou quatre paliers, et ce sont les paliers qui
//! empêchent de lire la distance.
//!
//! **Rien ici ne touche à la fenêtre.** La causalité va dans un seul sens : le système
//! la dimensionne, le jeu en déduit sa résolution. L'inverse — ajuster la fenêtre pour
//! qu'elle tombe juste — reprendrait à qui joue le geste qu'il vient de faire.

use screengine_play::MAX_RESOLUTION;

#[cfg(test)]
mod tests;

/// L'ouverture : la résolution de la première image, avant tout pas de jeu.
///
/// **Elle n'est pas le plafond**, et c'est ce qui rend la suite possible : sans un
/// plafond réglé plus haut à la création, l'ouverture *serait* le plafond et aucune
/// hausse ne passerait. C'est aussi la cote où les cotes de l'interface ont été réglées
/// à l'œil, et l'étage d'accueil ouvre sa fenêtre au grain près — si bien que la
/// première image que l'on voit est exactement celle-ci.
pub const BASE: (u32, u32) = (640, 360);

/// Le facteur d'agrandissement minimum : le grain de l'époque visée.
///
/// **C'est lui qui porte le budget d'image, et non le plafond.** `docs/conception.md`
/// range la résolution interne basse remontée en entier parmi ce que ce jeu existe pour
/// montrer : c'est un **facteur**, pas une cote, et l'écrire comme un facteur est ce
/// qui le garde sur n'importe quel écran. Deux donne un pixel de quatre à l'écran.
///
/// **Et c'est le seul nombre à reprendre si la cadence casse.** Le coût d'une image
/// suit ses pixels, donc passer à trois les divise par plus de deux ; le relevé de la
/// barre de titre dit lequel choisir, et rien d'autre ne le dira.
pub const GRAIN: u32 = 2;

/// Ce que la résolution interne ne dépasse jamais : la borne du moteur, sur les deux
/// axes.
///
/// **Reprise telle qu'elle est publiée, jamais recopiée.** Le moteur la donne en pixels
/// de côté, et elle vaut pour la largeur comme pour la hauteur ; l'écrire en clair ici
/// la figerait au jour où on l'écrit, et le calcul se mettrait à mentir si elle bougeait
/// là-bas.
///
/// **Ce n'est plus le plafond du contexte, c'est la borne de ce calcul** — et les deux
/// se sont séparés quand l'étage d'accueil a su prendre le sien des écrans. Celle-ci
/// reste parce que `fit` ne doit rien rendre que le moteur refuserait, et elle est la
/// seule cote dont ce module soit sûr sans rien connaître de la machine.
///
/// **Elle ne mord en pratique jamais**, le grain s'en chargeant avant : il faudrait une
/// fenêtre de plus de 4096 pixels de côté. C'est une garde, pas un réglage.
pub const CEILING: (u32, u32) = (MAX_RESOLUTION, MAX_RESOLUTION);

/// Ce sous quoi elle ne descend pas, une fenêtre pouvant être réduite à rien.
///
/// Sous cette cote, l'image est plus grande que la fenêtre et la mise à l'échelle
/// retombe sur son repli, qui garde les proportions. C'est préférable à une image de
/// quelques pixels de côté, où plus rien ne se lit.
pub const FLOOR: (u32, u32) = (320, 180);

/// La résolution interne qu'une fenêtre de cette taille réclame.
///
/// **Le grain d'abord, le plafond ensuite** : le facteur est au moins celui du grain, et
/// monte si la fenêtre dépasse ce que le moteur accepte — ce qui n'arrive qu'au-delà de
/// 4096 pixels de côté, le grain couvrant tout le reste. Les deux bornes se lisent donc
/// dans le même `max`, et c'est la seconde qui rend un refus impossible.
///
/// **Un facteur commun aux deux axes**, et non un par axe : c'est ce qui garde l'aspect
/// de la fenêtre, et un facteur par axe étirerait tout ce qui est rond sans jamais
/// paniquer ni sortir du plafond.
///
/// **Entier, parce que c'est ce que la mise à l'échelle rend sans filtrer** : un facteur
/// fractionnaire lui fait prendre son repli, et le grain net est ce que ce jeu montre.
///
/// **Le plancher est appliqué après le facteur**, donc il peut rompre l'aspect — mais
/// seulement sous une cote où l'image dépasse déjà la fenêtre, où il n'y a plus
/// d'aspect à tenir.
pub fn fit(window: (u32, u32)) -> (u32, u32) {
    let factor = GRAIN
        .max(window.0.div_ceil(CEILING.0))
        .max(window.1.div_ceil(CEILING.1));

    (
        (window.0 / factor).max(FLOOR.0),
        (window.1 / factor).max(FLOOR.1),
    )
}
