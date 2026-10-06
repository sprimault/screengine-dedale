// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Une créature : un corps que le décor arrête, et une planche qui la montre.
//!
//! **Le volume et le dessin sont deux choses**, et les confondre est le piège de
//! ce module : un marcheur aussi large que son dessin ne passerait nulle part. La
//! boîte qui se balaie tient dans un couloir ; la vignette qui se soumet déborde
//! d'elle, parce qu'un démon a des bras, des cornes et une ombre portée.
//!
//! **Le sprite est axial et non plein face.** Un personnage debout regardé d'en
//! haut se couche avec le plein face, ce qui est une image fausse ; en axial il
//! s'aplatit, ce qui n'est qu'une silhouette perdue. C'est l'inverse du choix de
//! l'arme, qui est plein face — donc rien ne se factorise entre les deux.
//!
//! **Il ne porte aucune normale**, et ce n'est pas un manque à combler : un
//! quadrilatère orienté caméra n'en a pas et n'en prend pas. Une créature garde
//! l'atténuation par la distance seule.

use std::sync::Arc;

use screengine_play::{
    Affine3, Angle, Camera, Color, Context, Error, Sprite, SpriteOrientation, Texture, Triangle,
    Vec3, VertexUv, World, load_png_masked,
};

use crate::body::Body;
use crate::maze::grid::{Grid, Side};
use crate::sheet::{self, FRAME, Motion};

#[cfg(test)]
mod tests;

/// La planche du premier démon, au repos.
///
/// **Une seule silhouette pour l'instant** : les trois entrent avec le labyrinthe
/// peuplé, et ce qui se juge ici est qu'une vignette se découpe et se lise à
/// distance. Trois planches ne le diraient pas mieux.
const IDLE: &[u8] = include_bytes!("../assets/sprites/demon-d1-idle-8x8-64.png");

/// Celle de sa marche.
///
/// **Chargée bien que rien ne marche encore**, et ce n'est pas du poids mort : la
/// règle qui choisit le cycle est écrite ici, et une planche qui manquerait le jour
/// où une distance devient non nulle ferait défiler des poses de repos sur un
/// déplacement.
const WALK: &[u8] = include_bytes!("../assets/sprites/demon-d1-walk-8x8-64.png");

/// Les demi-étendues du **corps**, en unités de monde.
///
/// **Plus étroit que le joueur et de même hauteur** : il faut qu'il passe dans un
/// couloir et sous une porte, et c'est tout ce que le volume décide. Le brief de
/// l'étape borne un démon à `1,5` sur les deux axes horizontaux et `1,625` en
/// hauteur, donc il y a de la marge.
const HALF: Vec3 = Vec3::new(0.35, 0.35, 0.9);

/// La demi-étendue du **sprite**, en unités de monde et jamais en texels.
///
/// **Elle se règle à l'écran, et c'est la raison d'être de ce lot.** En texels, la
/// créature changerait de taille avec la résolution interne, qui est un réglage ;
/// en unités de monde, elle garde sa place dans le décor. La vignette étant carrée
/// et son cadrage identique d'une vue à l'autre, une seule demi-étendue sert les
/// deux axes et le quadrilatère ne se déforme pas quand il tourne.
///
/// Un peu moins d'un mètre : la vignette couvre donc près de deux mètres de haut,
/// ce qu'un démon qui dépasse l'homme demande une fois le cadrage retranché.
const SPRITE_HALF: f32 = 0.96;

/// Ce que la planche de cette silhouette laisse de vide sous les pieds, en texels.
///
/// **Mesurée sur la planche employée, et elle n'est pas celle qu'on croyait** : le
/// cadrage de la chaîne vise deux pixels, mais le rendu en laisse **trois** sous
/// les pieds de cette silhouette-ci — et deux sous ceux d'une autre. Une marge
/// supposée faisait donc flotter la créature d'un texel, soit trois centimètres à
/// l'échelle du décor.
///
/// **Elle appartient à la silhouette et non à la grille**, et c'est ce que la
/// mesure a montré : les trois démons ne se cadrent pas pareil. Elle rejoindra la
/// table des silhouettes quand les trois entreront, et
/// `la_marge_annoncee_est_celle_de_la_planche` la tient d'ici là.
const MARGIN: f32 = 3.0;

/// Le rayon de la tache d'ombre, en unités de monde.
///
/// **Plus étroite que la silhouette et plus large que le corps** : une tache à la
/// taille du dessin déborderait sur les dalles voisines dès qu'on longe un mur, une
/// tache à la taille de la boîte ne se verrait pas. Elle se juge à l'écran, comme
/// tout ce qui a une taille apparente.
const SHADOW_RADIUS: f32 = 0.5;

/// De combien la tache flotte au-dessus du sol, en unités de monde.
///
/// **Un décalage de géométrie et non un biais de profondeur**, et la nuance est
/// celle que le contrat pose : une tache posée dans le plan de la surface qu'elle
/// marque la marque, le test de profondeur tolérant la pente. Ce centimètre ne sert
/// donc pas à gagner le test — il sert à ne pas entrer dans la dalle quand le sol
/// est en pente sous la créature, le quadrilatère étant horizontal et la rampe non.
const SHADOW_LIFT: f32 = 0.01;

/// Ce que la tache laisse passer en son centre, sur 255.
///
/// **255 est le neutre de la modulation**, donc un texel blanc laisse le sol intact
/// et un texel noir l'éteint : une surface modulée assombrit ou ne fait rien, elle
/// n'éclaircit jamais. Un peu plus du cinquième ne noircit pas la dalle, ce qui
/// ferait un trou là où on veut un appui.
const SHADOW_CORE: f32 = 0x38 as f32;

/// Le côté de la texture de la tache, en texels.
///
/// **Une puissance de deux, que le moteur exige**, et petite : c'est un dégradé
/// radial, donc elle n'a aucun détail à porter et son filtrage fait le reste.
const SHADOW_SIDE: u32 = 64;

/// Le côté de cette texture en coordonnées de texture, qui se prennent en texels.
const SIDE: f32 = SHADOW_SIDE as f32;

/// Le cap de la créature, en radians depuis le +X.
///
/// **Fixe, et c'est ce qui rend les huit vues visibles.** Une créature qui se
/// tournerait vers le joueur ne montrerait jamais que sa vue de face, et la planche
/// entière resterait à prouver. Elle tournera quand elle marchera.
const FACING: f32 = 0.0;

/// Combien de temps un cycle de repos met à se jouer, en secondes.
///
/// **Le repos avance avec l'horloge, là où une marche avancera avec la distance
/// parcourue** : une créature immobile n'a aucune distance à offrir, et son cycle
/// doit pourtant tourner. Une seconde et demie pour huit trames donne une
/// respiration lente, qu'on ne confond pas avec un pas.
const IDLE_PERIOD: f32 = 1.5;

/// La distance parcourue pour un cycle de marche complet, en unités de monde.
///
/// **La marche avance avec la distance et non avec le temps**, comme le
/// balancement de l'arme : un cycle indexé sur l'horloge continuerait de défiler
/// contre un mur, et une créature arrêtée piétinerait sur place. Un peu moins de
/// deux mètres pour huit trames donne une foulée d'un quart de mètre.
const STRIDE: f32 = 1.9;

/// Une créature posée dans le décor.
///
/// **Un état de partie**, comme le joueur et l'arme : elle est jetée au
/// rechargement de la carte, et aucun de ses champs n'a sa place dans une structure
/// du monde.
pub struct Monster {
    /// Sa planche de repos, chargée une fois.
    idle: Arc<Texture>,
    /// Celle de sa marche.
    walk: Arc<Texture>,
    /// La tache qu'elle pose au sol.
    ///
    /// **Une par créature plutôt qu'une pour toutes**, parce qu'un `Arc` se
    /// partagera quand elles seront plusieurs : c'est la population qui décidera où
    /// la fabriquer, pas ce lot, et la déplacer alors ne coûte rien.
    shadow: Arc<Texture>,
    /// Son corps, qui porte sa pose et sa cellule.
    ///
    /// **Le même chemin que le joueur**, et c'est ce que l'extraction du corps a
    /// acheté : la pose se résout par un balayage, quelle que soit la forme du sol
    /// sous elle, et le gabarit n'est plus celui d'un autre.
    body: Body,
    /// Son cap, en radians, qui décide de la vue avec la pose de l'œil.
    facing: f32,
    /// Ce qu'elle est en train de faire.
    motion: Motion,
    /// Où en est son cycle, en tours.
    phase: f32,
}

impl Monster {
    /// Pose une créature près de l'entrée du labyrinthe.
    ///
    /// **Sur une case voisine du départ**, pour qu'elle soit en vue sans qu'on la
    /// cherche : ce lot existe pour être regardé, et un démon à vingt cellules de là
    /// ne dirait rien des planches.
    ///
    /// # Erreurs
    ///
    /// Si la planche ne se décode pas — elle est intégrée au binaire, donc jamais en
    /// pratique, mais un `expect` sur un chemin atteignable n'a pas sa place.
    pub fn new(grid: &Grid, map: &World) -> Result<Self, Error> {
        let start = grid.start();
        // Un labyrinthe est connexe, donc le départ a toujours un passage ; s'en
        // remettre au départ lui-même plutôt qu'à un `expect` évite d'ériger en
        // invariant ce qui n'est qu'une commodité de placement.
        let at = Side::ALL
            .into_iter()
            .filter(|side| !side.is_vertical())
            .find(|&side| !grid.has_wall(start, side))
            .and_then(|side| grid.neighbour(start, side))
            .unwrap_or(start);

        Ok(Self {
            idle: Arc::new(load_png_masked(IDLE)?),
            walk: Arc::new(load_png_masked(WALK)?),
            shadow: Arc::new(shadow_texture()),
            body: Body::stand(HALF, grid, map, at),
            facing: FACING,
            motion: Motion::Idle,
            phase: 0.0,
        })
    }

    /// Avance son cycle de ce qu'elle a parcouru, et du temps écoulé.
    ///
    /// **C'est la distance qui décide du cycle**, et la règle est écrite ici
    /// quoiqu'elle ne se déplace pas encore : un cycle choisi ailleurs serait à
    /// défaire le jour où elle marche, et une créature qui marche en montrant des
    /// poses de repos est précisément ce qu'on ne verrait pas tout de suite.
    ///
    /// **Changer de cycle remet la phase à zéro**, parce que les deux planches n'ont
    /// pas le même compte de trames et qu'une phase reportée tomberait au milieu
    /// d'un pas. Une créature qui s'arrête repart de sa première pose de repos.
    pub fn advance(&mut self, travel: f32, dt: f32) {
        let motion = match travel > 0.0 {
            true => Motion::Walk,
            false => Motion::Idle,
        };
        if motion != self.motion {
            self.motion = motion;
            self.phase = 0.0;
        }

        self.phase += match motion {
            Motion::Walk => travel / STRIDE,
            _ => dt / IDLE_PERIOD,
        };
    }

    /// La planche que son mouvement courant lui donne.
    fn sheet(&self) -> &Arc<Texture> {
        match self.motion {
            Motion::Walk => &self.walk,
            // La planche de mort se charge avec ce qui tue, à l'étape 4 ; rien ne
            // construit ce mouvement avant elle, et le bras n'est là que pour
            // l'exhaustivité du filtrage.
            Motion::Idle | Motion::Dead => &self.idle,
        }
    }

    /// Où son centre se trouve, ce que le titre de la fenêtre affiche.
    pub fn at(&self) -> Vec3 {
        self.body.centre()
    }
}

/// La texture de la tache d'ombre : sombre au centre, **blanche au bord**.
///
/// **Blanche et non transparente**, parce que 255 est le neutre de la modulation :
/// un texel blanc laisse le sol intact, et la tache s'éteint d'elle-même sur son
/// pourtour sans qu'on ait à la découper. La transparence binaire du moteur
/// donnerait un bord franc, qui se lirait comme un disque posé.
///
/// **Engendrée et non chargée** : c'est une forme, et une forme s'écrit. Une
/// planche de plus dans `assets/` demanderait à la chaîne de produire ce que
/// quatre lignes décrivent exactement.
fn shadow_texture() -> Texture {
    let side = SHADOW_SIDE;
    let mut bytes = Vec::with_capacity((side * side) as usize * 4);
    let half = side as f32 / 2.0;

    for v in 0..side {
        for u in 0..side {
            let (dx, dy) = (u as f32 + 0.5 - half, v as f32 + 0.5 - half);
            // Le carré du rayon normalisé : la racine ne servirait à rien, la
            // courbe voulue étant justement quadratique — une ombre dense sous le
            // corps et qui s'efface vite.
            let fade = ((dx * dx + dy * dy) / (half * half)).min(1.0);
            let level = (SHADOW_CORE + (255.0 - SHADOW_CORE) * fade) as u8;
            bytes.extend_from_slice(&[level, level, level, 0xFF]);
        }
    }

    Texture::load(side, side, &bytes)
        .unwrap_or_else(|_| unreachable!("carrée, puissance de deux, et de la bonne longueur"))
}

/// Les quatre coins de la tache d'une créature, dans le sens qui la rend visible.
///
/// **Le sens décide de la face vue**, et c'est le piège du projet : la caméra
/// neutre regarde le +X, son axe droit est le −Y et son haut le +Z. Décrit dans
/// l'autre sens, le quadrilatère est un dos de face et disparaît — sans erreur, et
/// sans rien à l'écran.
fn shadow_corners(centre: Vec3) -> [Vec3; 4] {
    let z = centre.z - HALF.z + SHADOW_LIFT;
    let corner = |dx: f32, dy: f32| Vec3::new(centre.x + dx, centre.y + dy, z);

    [
        corner(-SHADOW_RADIUS, -SHADOW_RADIUS),
        corner(SHADOW_RADIUS, -SHADOW_RADIUS),
        corner(SHADOW_RADIUS, SHADOW_RADIUS),
        corner(-SHADOW_RADIUS, SHADOW_RADIUS),
    ]
}

/// Où poser le centre du quadrilatère pour que les pieds touchent le sol.
///
/// **Ce n'est pas le centre du corps**, et c'est ce que la fonction existe pour
/// dire : le dessin monte plus haut que la boîte, et la planche laisse du vide
/// sous les pieds. Le centre monte donc de la demi-étendue du sprite, moins ce que
/// le cadrage a laissé — un retrait qui est la même fraction de la vignette en
/// unités de monde qu'en texels.
fn anchor(centre: Vec3) -> Vec3 {
    let lift = SPRITE_HALF - MARGIN / FRAME * (2.0 * SPRITE_HALF);

    Vec3::new(centre.x, centre.y, centre.z - HALF.z + lift)
}

/// Soumet les créatures, **après le décor et avant l'arme**.
///
/// **Les taches d'abord, et c'est la seule contrainte d'ordre qui reste.** Une
/// surface modulée multiplie le tampon : elle ne peut assombrir que ce qui y est
/// déjà, donc le décor doit avoir été soumis. Elle teste la profondeur sans
/// l'écrire, et son test tolère la pente, si bien qu'elle gagne sur les dalles par
/// le seul ordre de soumission — sans biais de profondeur, qui vaudrait des
/// millimètres de près et des mètres au loin.
///
/// **Les sprites, eux, ne s'ordonnent pas** : la transparence du moteur est binaire,
/// donc le z-buffer tranche dans n'importe quel ordre et aucun tri n'est à faire. Ce
/// que leur ordre évite est qu'un décor très proche les rejette à égalité, et l'arme
/// reste la dernière parce qu'elle est la plus proche de l'œil.
///
/// **Le centre du quadrilatère n'est pas celui du corps.** Les pieds doivent
/// toucher le sol, et la planche laisse du vide sous eux : le centre monte donc de
/// la demi-étendue du sprite, moins ce que le cadrage a laissé. Poser le sprite sur
/// le centre du corps ferait flotter une créature haute et enterrerait une basse.
///
/// Comme la scène et l'arme, cette fonction ne lit ni horloge, ni entrée, ni tampon
/// de sortie : le chemin de rendu hors fenêtre de l'étape 8 l'appellera telle
/// quelle.
pub fn submit(
    context: &mut Context,
    monsters: &[&Monster],
    camera: &Camera,
) -> Result<(), screengine_play::screengine::Error> {
    for monster in monsters {
        let corners = shadow_corners(monster.body.centre());
        let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
        let vertices: Vec<VertexUv> = corners
            .iter()
            .zip([(0.0, 0.0), (SIDE, 0.0), (SIDE, SIDE), (0.0, SIDE)])
            .map(|(&position, (u, v))| VertexUv { position, u, v })
            .collect();

        context.submit_blended(
            Affine3::IDENTITY,
            &vertices,
            &[
                Triangle {
                    indices: [0, 1, 2],
                    color: white,
                },
                Triangle {
                    indices: [0, 2, 3],
                    color: white,
                },
            ],
            // Pas de mode à passer : `submit_blended` **est** la modulation côté
            // Rust, là où la frontière C prend un `blend` qui lui laisse la place
            // d'un mode additif. Rien à choisir ici, donc rien à se tromper.
            Some(&monster.shadow),
        )?;
    }

    for monster in monsters {
        let centre = monster.body.centre();
        let row = sheet::row(monster.facing, centre, camera.position);
        let column = sheet::column(monster.motion, monster.phase);
        let (u0, v0, u1, v1) = sheet::rect(row, column);

        context.submit_sprites(
            Affine3::IDENTITY,
            &[Sprite {
                center: anchor(centre),
                half_width: SPRITE_HALF,
                half_height: SPRITE_HALF,
                u0,
                v0,
                u1,
                v1,
                // Une créature debout ne roule pas : le roulis sert une lueur ou une
                // étincelle, et il tournerait ici la silhouette dans son plan.
                roll: Angle::from_radians(0.0),
                color: Color::new(0xFF, 0xFF, 0xFF, 0xFF),
            }],
            Some(monster.sheet()),
            SpriteOrientation::Axial,
        )?;
    }

    Ok(())
}
