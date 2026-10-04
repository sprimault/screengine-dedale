// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le corps du joueur : son volume, sa pose, et la cellule qui le contient.
//!
//! **Ce qui se déplace est le centre d'un corps, jamais l'œil.** Un volume centré
//! sur l'œil flotterait au-dessus de ce qui est bas et le traverserait — c'est
//! géométriquement juste et parfaitement faux —, et l'œil se déduit donc du
//! corps par un décalage. Jamais l'inverse.
//!
//! **Le décor arrête le déplacement, et le corps glisse le long de ce qui
//! l'arrête.** Le moteur ne rend qu'un temps d'impact et une normale : la réponse
//! est une politique du jeu, et c'est [`Player::slide`] qui la porte.
//!
//! **Et un corps pris dans le solide en sort**, au lieu d'y traverser le décor :
//! le moteur signale le départ pénétrant sans jamais dégager, c'est sa clause, donc
//! la poussée est ici aussi.
//!
//! **Et la pesanteur ramène au sol**, ce qui fait du contact l'état normal : la
//! vitesse de chute vit dans le corps, le critère de repos est un triplet que
//! [`Player::grounded`] porte, et une chute glisse le long de ce qu'elle rencontre
//! comme un pas.
//!
//! Ce qui n'y est pas encore, et qui se voit en jouant : une marche arrête au lieu
//! de se franchir, et une pente continue se redescend faute d'un critère de surface
//! marchable.

use screengine_play::{Vec3, World, sweep_skin};

use crate::maze::export::{self, SLOPE};
use crate::maze::grid::Grid;

#[cfg(test)]
mod tests;

/// Les demi-étendues du corps, en unités de monde.
///
/// Soixante centimètres de côté sur un mètre quatre-vingt : de quoi passer dans
/// une cellule de trois mètres et sous un plafond de deux et demi, en gardant
/// l'empreinte qui dépasse un palier de cage — ce que [`SLOPE`] existe pour
/// corriger.
///
/// **C'est le gabarit que les épreuves de l'export emploient déjà**, et c'est la
/// raison de le garder : leur domaine de validité est celui-là, et un autre
/// gabarit sortirait de ce qui a été éprouvé sans que rien ne le dise.
pub const HALF: Vec3 = Vec3::new(0.3, 0.3, 0.9);

/// De combien l'œil est au-dessus du centre du corps.
///
/// Les pieds posés, l'œil se trouve donc à `HALF.z + EYE_ABOVE` du sol, soit un
/// mètre cinquante. Ce n'est pas une cote de la carte : elle décide de l'échelle
/// qu'on prête au décor, et elle s'est jugée à l'écran, cote lue dans le titre de
/// la fenêtre.
///
/// **Elle dépend du plafond et de ce qu'on tient en main, pas de l'anatomie.** Sous
/// un plafond de deux mètres cinquante, un mètre soixante paraissait trop haut — il
/// ne restait que quatre-vingt-dix centimètres au-dessus de l'œil. Sous trois
/// mètres vingt-cinq la même cote passait, et c'est l'arme en main qui a donné
/// l'échelle manquante : dix centimètres plus bas, le décor prend sa taille.
///
/// **Ni elle ni `HALF.z` ne sont exactes en binaire, et cela ne nuit pas** : elles
/// composent une pose de caméra, jamais une cote écrite dans la carte. L'exactitude
/// s'impose à ce qui s'apparie au bit près et à ce qui entre dans un volume signé,
/// pas à ce qui sert à regarder.
pub const EYE_ABOVE: f32 = 0.6;

/// Combien de plans une glissade consomme au plus dans un même pas.
///
/// **Trois sont consommés au pire, et c'est relevé** : un pas en diagonale
/// descendante dans le coin d'une cellule y présente deux murs et le sol dans le
/// même mouvement. `une_glissade_garde_sa_marge` le mesure sur toutes les cases et
/// les vingt-six directions, et exige que le relevé reste **sous** cette borne —
/// la quatrième passe est donc la marge, celle qui consomme ce qui reste quand les
/// trois plans ont servi.
///
/// **Le dépasser écourte un pas, il ne franchit rien** : chaque passe balaie, donc
/// la borne ne décide que du confort dans un coin, jamais de la solidité d'un mur.
const SLIDES: usize = 4;

/// En combien de marges se fait un pas de dégagement d'un départ solide.
///
/// **Dimensionné sur la marge du balayage plutôt que sur une distance choisie** :
/// une valeur en unités de monde serait juste pour ce corps et fausse pour un
/// monstre, là où la marge suit la taille de la boîte. Pour ce corps, cela fait
/// cinquante-six millimètres par image — l'ordre d'un pas de marche —, donc on sort
/// d'un palier en une quinzaine d'images sans être éjecté.
///
/// **Le dégagement est progressif, et il ne peut pas être autrement** : le moteur
/// rend la normale de la surface la moins pénétrée, jamais la profondeur de
/// pénétration. Rien ici ne peut donc décider d'un recul exact, et ce facteur est le
/// seul réglage de cette politique — le reste est géométrique.
const ESCAPE: f32 = 64.0;

/// Ce que la chute gagne en vitesse par seconde, en unités de monde.
///
/// **Deux fois la pesanteur, et c'est un réglage d'écran** : à 9,81 la chute d'un
/// étage paraît planer, ce que les jeux de cette famille corrigeaient déjà en
/// prenant une pesanteur plus franche. La cote se juge en tombant d'un palier, pas
/// en raisonnant.
const GRAVITY: f32 = 20.0;

/// La course de la sonde qui demande si le corps repose sur quelque chose.
///
/// **Dimensionnée par domination et non par égalité** : il suffit qu'elle dépasse
/// le flottement d'un corps posé — la marge que le balayage laisse, un millième de
/// la plus grande demi-étendue, soit neuf dix-millièmes ici —, pas qu'elle lui soit
/// égale. Deux centimètres laissent un ordre de grandeur, donc ce critère n'attend
/// aucune constante du moteur et ne se dérègle pas si la marge change.
const PROBE: f32 = 0.02;

// **Ce que le décor laisse de place, vérifié à la compilation.** Un corps plus
// large qu'une cellule coincerait dans un couloir, un corps plus haut que le
// plafond ne passerait nulle part, et l'œil sous le plafond est ce qui empêche de
// regarder à travers. Ce sont des relations entre constantes : elles n'ont rien à
// faire dans une épreuve, qui ne les vérifierait qu'après la compilation.
const _: () = assert!(2.0 * HALF.x < export::INNER);
const _: () = assert!(2.0 * HALF.y < export::INNER);
const _: () = assert!(2.0 * HALF.z < export::CEILING);
const _: () = assert!(HALF.z + EYE_ABOVE < export::CEILING);

/// Le joueur : un volume, une pose, et la cellule où il se trouve.
pub struct Player {
    /// Le centre du corps.
    centre: Vec3,
    /// La cellule qui le contient, ou zéro s'il est hors du décor.
    cell: u32,
    /// Où son centre était au pas précédent, ce dont le suivi a besoin.
    previous: Vec3,
    /// La vitesse verticale, négative en descente.
    ///
    /// **Elle vit dans le corps et non dans la partie**, et la frontière tient : ce
    /// n'est pas un compteur de jeu mais l'état d'un mouvement, comme la pose
    /// elle-même. Un rechargement de carte repose le corps, donc la remet à zéro
    /// avec lui.
    fall: f32,
}

impl Player {
    /// Pose le joueur à l'entrée du labyrinthe, debout sur son sol.
    ///
    /// **La cellule vient de l'export et non d'une localisation** : `cover` la
    /// rend par la seule position de la case, là où `locate` parcourt toutes les
    /// cellules et toutes leurs faces pour retrouver ce que l'export sait déjà.
    ///
    /// **La pose se résout par un balayage, elle ne se calcule pas.** Le corps part
    /// assez haut pour qu'aucune pente du décor ne le pénètre et descend jusque
    /// **sous** la cote du sol : le balayage rencontre donc toujours quelque chose,
    /// et la fraction rendue pose le corps juste au-dessus, quelle que soit la
    /// forme du sol — plat, en marches ou en rampe. Calculer cette cote
    /// demanderait de savoir laquelle des trois, et c'est ce que le jeu n'a pas à
    /// connaître.
    ///
    /// **Viser sous le sol plutôt qu'au ras** n'est pas une précaution : une cible
    /// posée un quart de millimètre au-dessus tombe dans la bande de contact du
    /// moteur, le balayage n'y rencontre rien, et le corps resterait là où le
    /// calcul l'a mis — c'est-à-dire là où on voulait précisément ne pas décider.
    pub fn spawn(grid: &Grid, map: &World) -> Self {
        Self::stand(grid, map, grid.start())
    }

    /// Pose le joueur debout sur une case donnée.
    ///
    /// **Le départ n'est qu'un cas particulier**, et c'est ce qui rend la pose
    /// éprouvable ailleurs qu'à l'entrée : un prédicat de déplacement a besoin de
    /// se placer où il veut, et recopier ce calcul dans les épreuves le ferait
    /// diverger de celui du jeu.
    pub fn stand(grid: &Grid, map: &World, at: (u32, u32, u32)) -> Self {
        let cell = export::cover(grid, at);
        let spot = export::ground(grid, at);

        // Le dégagement qu'une pente impose : le coin aval du bas monte autant
        // que le corps est profond, fois la pente.
        let clear = HALF.x * SLOPE;
        let above = Vec3::new(spot[0], spot[1], spot[2] + HALF.z + clear);
        let below = Vec3::new(above.x, above.y, spot[2] + HALF.z - clear);

        let centre = match map.sweep(cell, HALF, above, below) {
            Some(hit) if !hit.start_solid => above + (below - above) * hit.fraction,
            // Un départ que le décor ne devrait pas offrir : on garde la cote
            // haute, la moins pénétrante des deux, et l'épreuve de pose dit que
            // le cas est arrivé.
            _ => above,
        };

        Self {
            centre,
            cell,
            previous: centre,
            fall: 0.0,
        }
    }

    /// Vrai si le corps **repose** sur quelque chose.
    ///
    /// **Trois conditions, et aucune ne suffit seule.** Un départ dans le solide dit
    /// qu'on est dedans, pas qu'on est posé — c'est le piège de ce critère, et il
    /// disqualifie donc. Une fraction nulle sans normale verticale est un mur qu'on
    /// touche de côté. Et une normale verticale rencontrée plus loin est un sol vers
    /// lequel on tombe, pas un sol sur lequel on est.
    ///
    /// **Ce qui rend ce critère possible est le flottement** : le balayage pose un
    /// corps à sa marge du sol, donc la sonde rencontre ce sol aussitôt — à quatre
    /// centièmes de sa course, pas à zéro exact, et c'est pourquoi la comparaison
    /// porte sur « avant le bout » plutôt que sur une égalité.
    ///
    /// **La course est donc aussi une tolérance, et c'est voulu** : un corps à moins
    /// de deux centimètres du sol est tenu pour posé. Sans elle, le moindre
    /// flottement au passage d'un joint ferait repartir une chute d'une image, ce qui
    /// se verrait comme un tremblement.
    fn grounded(&self, map: &World) -> bool {
        let below = Vec3::new(self.centre.x, self.centre.y, self.centre.z - PROBE);

        map.sweep(self.cell, HALF, self.centre, below)
            .is_some_and(|hit| !hit.start_solid && hit.fraction < 1.0 && hit.normal.z > 0.5)
    }

    /// Déplace le corps de ce qu'il peut parcourir, et suit sa cellule.
    ///
    /// **Le décor arrête désormais le déplacement**, et ce qui est rendu est la
    /// distance réellement franchie : l'appelant s'en sert pour ce qui dépend de
    /// la marche, et elle vaut zéro contre un mur.
    ///
    /// Le suivi porte sur le **centre du corps** et non sur l'œil, parce que c'est
    /// lui qui a un volume et que c'est son volume qu'on balaie. Le moteur ne se
    /// relocalise jamais de lui-même : zéro veut dire « sorti du décor », et c'est
    /// à l'hôte de le redemander.
    pub fn advance(&mut self, map: &World, moved: Vec3, dt: f32) -> f32 {
        // **La pesanteur s'intègre avant le pas, et la chute en fait partie** : un
        // seul balayage par image porte les deux, donc ce qui tombe glisse le long
        // de ce qu'il rencontre au lieu de s'y arrêter net.
        self.fall -= GRAVITY * dt;
        let wanted = self.centre + moved + Vec3::new(0.0, 0.0, self.fall * dt);

        let (reached, _) = self.slide(map, wanted);
        let travel = reached - self.centre;

        self.centre = reached;
        let found = map.track(self.cell, self.previous, self.centre);
        self.cell = if found == 0 {
            map.locate(self.centre)
        } else {
            found
        };
        self.previous = self.centre;

        // **Posé, la vitesse de chute repart de zéro.** Sans cela elle croîtrait
        // pendant toute la marche, et le premier bord franchi donnerait une chute de
        // plusieurs mètres en une image — un corps téléporté vers le bas.
        if self.grounded(map) {
            self.fall = 0.0;
        }

        travel.dot(travel).sqrt()
    }

    /// Jusqu'où le corps va, en glissant le long de ce qui l'arrête.
    ///
    /// **Chaque passe avance jusqu'au contact, puis retire du reste sa composante
    /// sur la normale** : ce qui restait d'un pas oblique continue le long du mur
    /// au lieu de se perdre. C'est toute la politique, et le moteur n'en connaît
    /// aucune — il rend un temps d'impact et une normale.
    ///
    /// **La normale ne tremble pas, et c'est ce qui rend la boucle écrivable.** Le
    /// moteur classe les arêtes partagées au chargement, par comparaison exacte
    /// des positions, et une arête rentrante ne porte aucun prisme : un angle
    /// rentrant rend donc toujours la même face. Sans cela, deux images voisines
    /// glisseraient le long de deux murs différents.
    ///
    /// **Rien ne garde contre un recul, et c'est une mesure, pas un oubli.** Après
    /// une projection unique, le reste ne peut pas s'opposer au pas demandé —
    /// l'inégalité de Cauchy-Schwarz le donne —, et seul un enchaînement de deux
    /// projections le pourrait, ce qui demande un dièdre aigu. Ce décor n'en porte
    /// aucun, ni par ses murs axiaux ni par une rampe à quarante-cinq degrés, et
    /// `la_glissade_ne_fait_jamais_reculer` ne déclenche le cas sur aucune case dans
    /// aucune des vingt-six directions. Un test de signe y serait de la défensive
    /// autour de ce qui n'arrive pas, et projeter sur l'arête des deux normales n'a
    /// pas plus de demandeur.
    ///
    /// **Trois cas avant toute politique, et aucun n'est un détail** :
    ///
    /// - **hors du décor**, la cellule valant zéro : rien à balayer, et refuser le
    ///   déplacement enfermerait dehors. Le zéro se teste ici parce que l'API ne
    ///   le distingue pas d'un identifiant inconnu — les deux rendent `None`, là
    ///   où la frontière C les sépare ;
    /// - **une cellule que la carte ne connaît pas**, qui ne devrait pas arriver
    ///   puisqu'elle vient du suivi : le déplacement passe plutôt que de bloquer
    ///   sur une erreur de comptabilité ;
    /// - **un départ dans le solide**, que le moteur signale sans dégager : le pas
    ///   ne s'applique pas et le corps est repoussé le long de la normale rendue,
    ///   d'un pas de [`ESCAPE`] marges. C'est la politique que l'exemple `carte` du
    ///   moteur a adoptée, donc celle que ses épreuves exercent — en prendre une
    ///   autre priverait de ce qu'elles couvrent.
    ///
    /// Rend le point atteint **et le nombre de plans consommés**, dont seule une
    /// épreuve se sert : c'est ce qui mesure [`SLIDES`] au lieu de le supposer.
    fn slide(&self, map: &World, wanted: Vec3) -> (Vec3, usize) {
        if self.cell == 0 {
            return (wanted, 0);
        }

        let mut at = self.centre;
        let mut rest = wanted - self.centre;

        for plane in 0..SLIDES {
            let Some(hit) = map.sweep(self.cell, HALF, at, at + rest) else {
                return (at + rest, plane);
            };
            if hit.start_solid {
                // **Sortir passe avant avancer**, donc le pas demandé ne s'applique
                // pas : le rendre libre enfoncerait davantage, l'image suivante
                // repartirait solide, et un seul départ fautif rendrait la collision
                // inopérante pour de bon — mesuré, le corps traversait les murs.
                return (at + hit.normal * (sweep_skin(HALF) * ESCAPE), plane);
            }

            at = at + rest * hit.fraction;
            // `fraction` et non `surface`, qui vaut zéro pour trois cas distincts
            // dont deux arrêtent — un portail non apparié, et une troncature.
            if hit.fraction >= 1.0 {
                return (at, plane);
            }

            rest = rest * (1.0 - hit.fraction);
            rest = rest - hit.normal * rest.dot(hit.normal);
        }

        (at, SLIDES)
    }

    /// Où l'œil se trouve, pose de la caméra.
    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.centre.x, self.centre.y, self.centre.z + EYE_ABOVE)
    }

    /// La cellule qui contient le corps, ou zéro.
    pub fn cell(&self) -> u32 {
        self.cell
    }
}
