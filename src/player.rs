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
//! **Et les étages se parcourent** : une surface se marche ou ne se marche pas, selon
//! un seuil que [`walkable`] dérive de la pente du décor, et ce qui ne se marche pas
//! se franchit quand une marche suffit — [`Player::climb`] l'essaie en balayant,
//! jamais en devinant la hauteur d'un obstacle.
//!
//! Ce qui n'y est pas encore, et qui se juge à l'œil : le pas de côté, et les
//! réglages de la marche.

use screengine_play::{Vec3, World, sweep_skin};

use crate::maze::export::{self, RISE, SLOPE};
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

/// Vrai si l'on tient debout sur une surface de cette normale.
///
/// **Le seuil se dérive de la pente du décor, et c'est ce qui le rend juste** :
/// `SLOPE` est la plus raide que l'export produise, donc `1/√(1+SLOPE²)` — trois
/// cinquièmes ici — est la normale la plus couchée qu'un sol légitime puisse
/// présenter. Ce que le décor porte est marchable **par construction**, là où un
/// nombre choisi à la main serait juste pour ce décor et faux au suivant.
///
/// **Écrit en carrés plutôt qu'en racine**, et pas seulement pour éviter un calcul :
/// une racine n'est pas évaluable en constante, donc le seuil serait devenu une
/// valeur d'exécution qu'on aurait fini par recopier. L'inégalité est la même pour
/// une normale qui regarde le haut.
///
/// Une rampe monte l'étage sur le côté d'une case, donc `7/6`, et sa normale a
/// `6/√85 ≈ 0,651` : marchable avec de la marge. Un mur et une contremarche ont
/// zéro, et c'est ce qui fait qu'une marche se franchit au lieu de se gravir.
fn walkable(normal: Vec3) -> bool {
    normal.z > 0.0 && normal.z * normal.z * (1.0 + SLOPE * SLOPE) >= 1.0
}

// **Ce que le décor laisse de place, vérifié à la compilation.** Un corps plus
// large qu'une cellule coincerait dans un couloir, un corps plus haut que le
// plafond ne passerait nulle part, et l'œil sous le plafond est ce qui empêche de
// regarder à travers. Ce sont des relations entre constantes : elles n'ont rien à
// faire dans une épreuve, qui ne les vérifierait qu'après la compilation.
const _: () = assert!(2.0 * HALF.x < export::INNER);
const _: () = assert!(2.0 * HALF.y < export::INNER);
const _: () = assert!(2.0 * HALF.z < export::CEILING);
const _: () = assert!(HALF.z + EYE_ABOVE < export::CEILING);

/// Ce qu'une glissade a donné.
///
/// **Trois champs dont deux ne servent qu'une fois**, et c'est pourquoi ils ne sont
/// pas un tuple : un `(Vec3, usize, bool)` se relirait mal au troisième appelant, et
/// les noms disent ce que l'ordre ne dit pas.
struct Slid {
    /// Le point atteint.
    at: Vec3,
    /// Combien de plans la glissade a consommés.
    ///
    /// **Seule une épreuve s'en sert**, et c'est elle qui mesure [`SLIDES`] au lieu
    /// de le supposer — d'où l'exception au compte des champs morts, qui vaut pour la
    /// construction du jeu et non pour celle des épreuves. Le retirer rendrait la
    /// borne supposée.
    #[cfg_attr(not(test), allow(dead_code))]
    planes: usize,
    /// Vrai si une surface que [`walkable`] refuse a arrêté le pas avant son terme.
    ///
    /// C'est le signal du franchissement, et il est volontairement grossier : un mur
    /// et une contremarche le lèvent tous deux, et c'est le relèvement qui les
    /// sépare — par la géométrie, pas par une hauteur devinée.
    stopped: bool,
}

impl Slid {
    /// Le pas entier, sans rien avoir rencontré.
    fn free(at: Vec3) -> Self {
        Self {
            at,
            planes: 0,
            stopped: false,
        }
    }
}

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
    /// disqualifie donc. Un contact immédiat sur une surface que [`walkable`] refuse
    /// est un mur qu'on touche de côté, ou la contremarche qu'on va franchir. Et une
    /// surface marchable rencontrée au bout de la sonde est un sol vers lequel on
    /// tombe, pas un sol sur lequel on est.
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
            .is_some_and(|hit| !hit.start_solid && hit.fraction < 1.0 && walkable(hit.normal))
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
        // **La pesanteur ne s'applique qu'en l'air, et c'est ce qui tient sur une
        // pente.** Intégrée aussi quand le corps repose, elle vaut toujours un pas
        // vers le bas au moment du balayage : sur un plan horizontal la projection
        // l'annule, mais sur une rampe elle en garde la composante le long de la
        // pente, et le corps redescend un peu à chaque image.
        let was_grounded = self.grounded(map);
        if was_grounded {
            self.fall = 0.0;
        } else {
            self.fall -= GRAVITY * dt;
        }
        let wanted = self.centre + moved + Vec3::new(0.0, 0.0, self.fall * dt);

        let slid = self.slide(map, self.centre, wanted);
        let reached = if slid.stopped {
            // Buté sur ce qui ne se marche pas : c'est peut-être une marche, et
            // c'est le relèvement qui le dit — il balaie, là où une hauteur
            // calculée supposerait la forme de l'obstacle.
            let step = Vec3::new(moved.x, moved.y, 0.0);
            self.climb(map, step, slid.at).unwrap_or(slid.at)
        } else {
            slid.at
        };
        // **Et le pendant du franchissement : on descend d'une marche comme on en
        // monte une.** Un pas qui quitte le contact sans qu'il y ait de vide dessous
        // laisserait la pesanteur reprendre, donc descendre un escalier ou une rampe
        // serait une chute et non une marche — mesuré à l'écran, et c'est l'écart
        // entre un pas de 0,05 et ce que le sol perd dans le même pas : 0,058 sur une
        // rampe, 0,25 au bord d'un giron, l'un comme l'autre au-delà de la tolérance
        // de contact.
        let reached = match was_grounded {
            true => self.settle(map, reached).unwrap_or(reached),
            false => reached,
        };
        let travel = reached - self.centre;

        self.centre = reached;
        let found = map.track(self.cell, self.previous, self.centre);
        self.cell = if found == 0 {
            map.locate(self.centre)
        } else {
            found
        };
        self.previous = self.centre;

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
    /// **Le départ est un paramètre et non la pose du corps**, ce qui sert au
    /// franchissement : [`Player::climb`] rejoue le même pas depuis une position
    /// relevée, et il n'y a qu'une politique de glissade à tenir.
    fn slide(&self, map: &World, from: Vec3, wanted: Vec3) -> Slid {
        if self.cell == 0 {
            return Slid::free(wanted);
        }

        let mut at = from;
        let mut rest = wanted - from;
        let mut stopped = false;

        for plane in 0..SLIDES {
            let Some(hit) = map.sweep(self.cell, HALF, at, at + rest) else {
                return Slid {
                    at: at + rest,
                    planes: plane,
                    stopped,
                };
            };
            if hit.start_solid {
                // **Sortir passe avant avancer**, donc le pas demandé ne s'applique
                // pas : le rendre libre enfoncerait davantage, l'image suivante
                // repartirait solide, et un seul départ fautif rendrait la collision
                // inopérante pour de bon — mesuré, le corps traversait les murs.
                let out = at + hit.normal * (sweep_skin(HALF) * ESCAPE);
                return Slid {
                    at: out,
                    planes: plane,
                    stopped: false,
                };
            }

            at = at + rest * hit.fraction;
            // `fraction` et non `surface`, qui vaut zéro pour trois cas distincts
            // dont deux arrêtent — un portail non apparié, et une troncature.
            if hit.fraction >= 1.0 {
                return Slid {
                    at,
                    planes: plane,
                    stopped,
                };
            }

            // Ce qui ne se marche pas peut être une marche : c'est le seul signal
            // dont le franchissement a besoin, et il ne dit pas de quoi il s'agit.
            stopped |= !walkable(hit.normal);

            // **On repart hors de la bande de contact, d'une demi-marge le long de la
            // normale.** Posé pile à la distance que le balayage rend, un mouvement
            // **tangent** à une face oblique est refusé : la projection y laisse un
            // résidu d'arrondi de l'ordre de 3e-5, dont le signe décide, et la passe
            // suivante repart alors sur une fraction nulle — les quatre passes brûlent
            // sans avancer d'un millième. Mesuré en montant une rampe de biais : à
            // l'écart près, le même pas passe en entier.
            //
            // Une face axiale n'en avait pas besoin, et c'est ce qui l'a caché : sa
            // normale annule la composante exactement, sans résidu.
            at = at + hit.normal * (sweep_skin(HALF) * 0.5);

            rest = rest * (1.0 - hit.fraction);
            rest = rest - hit.normal * rest.dot(hit.normal);
        }

        Slid {
            at,
            planes: SLIDES,
            stopped,
        }
    }

    /// Repose le corps sur ce qui est à moins d'une marche sous lui.
    ///
    /// **Le pendant du franchissement, et la même constante** : on monte d'une
    /// marche, donc on descend d'une marche. Sans cela un pas qui quitte le contact
    /// rend la main à la pesanteur, et une descente d'escalier ou de rampe devient
    /// une chute accélérée au lieu d'une marche — c'est ce qui s'est vu à l'écran.
    ///
    /// **Ce qu'il ne fait pas, et c'est ce qui garde la chute** : au-delà d'une
    /// marche il n'y a plus de sol sous les pieds mais du vide, et tomber est alors
    /// la réponse juste. Le balayage tranche, et une surface que [`walkable`] refuse
    /// ne retient pas davantage.
    ///
    /// **Il ne s'appelle que si le corps était posé avant le pas** : un corps déjà en
    /// l'air est en train de tomber, et le coller au premier sol à portée
    /// interromprait sa chute d'une marche avant la fin.
    ///
    /// **Rend `None` quand il n'y a rien à moins d'une marche**, et c'est ce dont le
    /// franchissement se sert pour se refuser : relever un corps qui redescend un
    /// escalier le laissait en l'air d'une marche entière, hors de portée de ce
    /// collage — un franchissement qui ne repose pas n'en est pas un.
    fn settle(&self, map: &World, from: Vec3) -> Option<Vec3> {
        // **Une marche et deux marges** : une pour le contact que le balayage laisse,
        // une pour l'écart que la glissade ajoute en repartant du plan. Avec une
        // seule, la sonde s'arrêtait un demi-millième au-dessus du giron inférieur et
        // le corps quittait le sol au bord de la première marche — mesuré.
        let down = Vec3::new(from.x, from.y, from.z - (RISE + 2.0 * sweep_skin(HALF)));

        match map.sweep(self.cell, HALF, from, down) {
            Some(hit) if !hit.start_solid && hit.fraction < 1.0 && walkable(hit.normal) => {
                Some(from + (down - from) * hit.fraction)
            }
            _ => None,
        }
    }

    /// Tente de franchir ce qui vient d'arrêter le pas, en montant d'une marche.
    ///
    /// **Trois balayages, et c'est la forme qui décide** : on monte de [`RISE`], on
    /// rejoue le pas horizontal depuis là, puis on redescend d'autant. Rien n'y
    /// suppose la forme de l'obstacle — ni sa hauteur, ni qu'il soit une marche —, et
    /// la descente finale repose le corps sur le giron au lieu de le laisser flotter,
    /// ce qui ferait monter un escalier en sautillant.
    ///
    /// **Le pas rejoué est horizontal**, la chute de cette image ayant déjà servi :
    /// la rejouer ferait descendre ce qu'on vient de relever.
    ///
    /// **Et le gain se mesure plutôt que de se supposer** : le relèvement n'est
    /// adopté que s'il avance davantage, le long du pas demandé, que l'arrêt qu'il
    /// remplace. C'est ce qui fait qu'un mur reste un mur — on y monte, on n'y
    /// avance pas, et l'arrêt est gardé.
    ///
    /// **Le prix est trois balayages par image le long d'un mur**, et c'est assumé :
    /// l'alternative serait de n'essayer qu'au-delà d'une fraction de pas perdue,
    /// c'est-à-dire un seuil choisi à la main pour décider ce qu'une marche est.
    fn climb(&self, map: &World, step: Vec3, instead: Vec3) -> Option<Vec3> {
        // **Une marche et la marge du balayage, pas une marche tout juste** : relevé
        // de `RISE` exactement, le bas du corps arrive au niveau du giron, que le
        // contact arrête aussitôt — mesuré, l'escalier ne montait pas d'un pouce. Ce
        // que cela tolère de plus est la marge elle-même, soit un millième de la
        // demi-étendue, donc rien qu'on puisse franchir d'autre.
        let lift = Vec3::new(0.0, 0.0, RISE + sweep_skin(HALF));
        let up = self.slide(map, self.centre, self.centre + lift);
        let over = self.slide(map, up.at, up.at + step);
        let down = self.slide(map, over.at, over.at - lift);

        let gained = (down.at - self.centre).dot(step);

        (gained > (instead - self.centre).dot(step)).then_some(down.at)
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
