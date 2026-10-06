// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La planche de vues : une grille de vignettes, et par où on y entre.
//!
//! **Une planche range ses directions en lignes et ses trames en colonnes**, et
//! c'est ce que ce module existe pour écrire : rien d'autre dans le dépôt ne le
//! dit, et le nom d'un fichier n'est pas une donnée. Trois informations ne se
//! déduisent d'aucun octet livré — que le cycle de mort ne boucle pas, le sens
//! dans lequel les lignes tournent, et combien de trames chaque cycle porte —, et
//! elles vivent ici ou nulle part.
//!
//! **Il ne connaît ni créature ni joueur**, seulement une planche et un angle.
//! C'est pourquoi les lignes ne portent pas de nom de direction : ce sont des
//! angles, et un point cardinal ferait entrer une convention de jeu dans la
//! lecture d'une donnée.
//!
//! **La cote ne décide de rien**, et c'est une conséquence de l'orientation
//! axiale : le moteur ne fait tourner le quadrilatère qu'autour du Z du monde,
//! donc regarder une créature d'en haut ne change pas la vue qu'on en a. Seule la
//! projection horizontale de l'écart compte.

use screengine_play::Vec3;

#[cfg(test)]
mod tests;

/// Le côté d'une vignette, en texels.
///
/// **Toutes les planches partagent cette taille**, mortes comme vivantes, et
/// `les_planches_tiennent_la_grille_annoncee` le vérifie sur les neuf fichiers du
/// dépôt plutôt que de s'en remettre à leur nom.
pub const FRAME: f32 = 64.0;

/// Le nombre de vues d'une planche : une ligne par direction, de 0° à 315°.
const VIEWS: u32 = 8;

/// Ce qu'une créature est en train de faire, donc quelle planche la montre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motion {
    /// Au repos, sans se déplacer.
    Idle,
    /// En marche.
    Walk,
    /// En train de mourir, puis morte.
    Dead,
}

impl Motion {
    /// Combien de trames son cycle porte, une colonne chacune.
    ///
    /// **Seize pour la mort contre huit pour les deux autres** : c'est le seul
    /// cycle qui ne se répète pas, donc le seul qui doive tenir entier dans sa
    /// planche plutôt que de se boucler. Mesuré sur les fichiers, pas lu dans leur
    /// nom.
    fn frames(self) -> u32 {
        match self {
            Self::Idle | Self::Walk => 8,
            Self::Dead => 16,
        }
    }

    /// Vrai si le cycle reprend à sa première trame une fois épuisé.
    ///
    /// **La mort ne boucle pas, et rien dans la planche ne le dit** : seize images
    /// d'une créature qui tombe se rejoueraient indéfiniment, et un cadavre se
    /// relèverait toutes les deux secondes. Elle garde sa dernière trame, qui est
    /// la pose au sol.
    fn loops(self) -> bool {
        self != Self::Dead
    }
}

/// La ligne de la planche : sous quel angle la créature est vue.
///
/// **C'est l'écart entre son cap et la direction qui va d'elle vers l'œil**,
/// jamais l'un des deux seul : une créature qui marche vers le nord et qu'on
/// regarde depuis le nord se montre de face, et la même regardée depuis le sud se
/// montre de dos. Zéro met donc l'œil droit devant elle, et la ligne 0 est la vue
/// de face ; un demi-tour donne la ligne 4, de dos.
///
/// **Les lignes tournent dans le sens horaire vu de dessus**, d'où l'écart pris
/// comme `facing − to_eye` et non l'inverse. C'est la convention de **nos**
/// planches, mesurée sur une image de contrôle : à cap nul et œil posé en `+Y`, la
/// créature doit regarder vers la gauche de l'image, et l'autre sens la montrait
/// tournée vers la droite.
///
/// **Elle n'est pas celle de l'exemple `couloir` du moteur**, qui prend l'écart
/// dans l'autre sens pour ses propres planches. Une convention de données ne se
/// déduit pas du code qui lit d'autres données, et c'est ce que cette ligne a
/// coûté : lues de travers, les vues restent régulières et la créature marche à
/// reculons.
///
/// **Arrondi au secteur le plus proche, et non tronqué.** La troncature
/// décalerait chaque vue d'un demi-secteur, soit vingt-deux degrés et demi : la
/// créature paraîtrait marcher de travers, de biais par rapport à son
/// déplacement, sans qu'aucune pose soit fausse pour autant.
///
/// `facing` est en radians, mesuré depuis le +X et croissant vers le +Y.
pub fn row(facing: f32, at: Vec3, eye: Vec3) -> u32 {
    let to_eye = (eye.y - at.y).atan2(eye.x - at.x);
    let turn = core::f32::consts::TAU;
    let relative = (facing - to_eye).rem_euclid(turn);

    // Le demi-secteur ajouté avant la troncature est ce qui fait l'arrondi, et le
    // modulo rattrape le tour entier qu'il produit au-delà du dernier secteur.
    ((relative / turn * VIEWS as f32) + 0.5) as u32 % VIEWS
}

/// La colonne de la planche : où en est le cycle.
///
/// `phase` se compte **en tours de cycle** : sa partie entière est le nombre de
/// cycles accomplis, sa partie fractionnaire la position dans celui qui court.
/// C'est à l'appelant de la faire avancer, et c'est voulu — une marche avance
/// avec la **distance parcourue**, comme le balancement de l'arme, là où un repos
/// et une mort avancent avec le **temps**. Un cycle de marche indexé sur l'horloge
/// continuerait de défiler contre un mur.
///
/// **Un cycle qui ne boucle pas garde sa dernière trame**, et c'est tout ce qui
/// sépare une mort des deux autres ici : au-delà de son tour, la colonne reste
/// celle de la pose au sol.
pub fn column(motion: Motion, phase: f32) -> u32 {
    let frames = motion.frames();

    match motion.loops() {
        // Le modulo garde contre le seul cas que `rem_euclid` laisse passer : une
        // phase à peine négative rend une fraction qui arrondit à un en `f32`.
        true => (phase.rem_euclid(1.0) * frames as f32) as u32 % frames,
        false => ((phase * frames as f32) as u32).min(frames - 1),
    }
}

/// Le rectangle de texture d'une vignette, en texels, dans l'ordre `u0 v0 u1 v1`.
///
/// **En texels et non en fraction de planche**, parce que c'est ce que le moteur
/// attend d'un sprite : le rectangle est porté par la soumission, ce qui permet à
/// une planche de servir toutes les créatures d'un lot sans changer de texture.
///
/// **`v` croît vers le bas**, donc la ligne 0 est en haut de la planche. C'est la
/// convention du moteur, et c'est aussi celle dans laquelle les planches sont
/// produites : il n'y a rien à retourner d'un côté ni de l'autre.
pub fn rect(row: u32, column: u32) -> (f32, f32, f32, f32) {
    let (u, v) = (column as f32 * FRAME, row as f32 * FRAME);
    (u, v, u + FRAME, v + FRAME)
}
