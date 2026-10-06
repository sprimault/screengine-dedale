// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la scène.

use super::*;
use crate::maze::grid::Side;
use crate::player::{EYE_ABOVE, HALF, Player};
use crate::test_support::{HEIGHT, WIDTH, context, frame};
use screengine_play::screengine::BYTES_PER_PIXEL;
use screengine_play::{Affine3, Angle, Camera, FreeCamera, Quat, Vec3, Visibility};

/// Les réglages du labyrinthe d'épreuve.
fn settings() -> Settings {
    Settings {
        extent: (16, 16, 2),
        seed: 0x5EED_1A8E,
        stairs: 6,
        ramps: 2,
        loops: 8,
    }
}

/// Un échantillon de poses dans le labyrinthe, chacune regardant les quatre
/// directions.
///
/// **Peu nombreuses et choisies, non tirées** : chaque pose coûte deux images dont
/// l'une soumet la carte entière, et ce qu'on cherche — une traversée trop étroite
/// — ne dépend pas du hasard mais de la profondeur du décor. Le départ et la
/// sortie sont les deux extrêmes de cette profondeur, par construction.
fn poses(scenery: &Scenery) -> Vec<Camera> {
    let mut out = Vec::with_capacity(8);
    for at in [scenery.maze.start(), scenery.maze.exit()] {
        let spot = export::ground(&scenery.maze, at);
        // L'œil d'un corps posé : sa demi-hauteur, plus le décalage de l'œil.
        let position = Vec3::new(spot[0], spot[1], spot[2] + HALF.z + EYE_ABOVE);
        for quarter in 0..4 {
            out.push(Camera {
                position,
                orientation: Quat::from_axis_angle(
                    Vec3::new(0.0, 0.0, 1.0),
                    Angle::from_radians(quarter as f32 * core::f32::consts::FRAC_PI_2),
                ),
                ..Camera::DEFAULT
            });
        }
    }
    out
}

/// Des poses serrées contre le plan d'un portail, de part et d'autre, le
/// regardant.
///
/// **C'est la configuration qui manquait à l'oracle.** Celui-ci compare bien les
/// deux chemins, mais il ne dit rien d'une pose qu'on ne lui donne pas : celles de
/// [`poses`] sont au point sûr d'une case, donc à plus d'un mètre de toute
/// embrasure, là où ce qui se joue tient à quelques centièmes d'unité.
///
/// **Les écarts encadrent le plan proche de la caméra**, qui est ce qui décide :
/// un portail entièrement en deçà s'escamote avec la cellule qu'il mène, un
/// portail que le plan coupe n'en perd qu'une part. Les deux se sont vus en
/// marchant, l'un vidant l'image et l'autre lui mangeant une bande.
///
/// **Et de biais autant que de face**, parce que de face un portail bascule d'un
/// coup : il faut l'angle pour qu'une moitié soit en deçà du plan et l'autre
/// au-delà. C'est de biais que la perte commence le plus tôt, avant même que
/// l'écart atteigne le plan proche, et de face qu'elle emporte tout.
fn poses_contre_un_portail(scenery: &Scenery) -> Vec<Camera> {
    /// La demi-épaisseur d'un mur, que l'export garde pour lui mais qui se
    /// retrouve : une cellule occupe `INNER` au milieu de sa case de `CELL`.
    const MARGIN: f32 = (export::CELL - export::INNER) / 2.0;

    /// Les écarts au plan du portail, en unités de monde.
    ///
    /// Le dixième est le plan proche par défaut du moteur, et les valeurs qui
    /// l'entourent sont ce qu'une marche à quatre unités par seconde
    /// n'échantillonne qu'une image sur deux. **Le cinquième d'unité est au-delà
    /// de la fenêtre** : il passe aujourd'hui, de face comme de biais, et c'est
    /// le témoin qui dit que l'épreuve ne condamne pas tout.
    const GAPS: [f32; 6] = [0.20, 0.12, 0.10, 0.09, 0.05, 0.02];

    let maze = &scenery.maze;
    let (width, height, _) = maze.extent();
    let flat = |at: (u32, u32, u32)| {
        maze.stairs()
            .iter()
            .all(|stair| stair.foot != at && stair.head() != at)
    };

    // Une case de l'étage du bas qui ouvre vers l'est, plate des deux côtés du
    // passage : sur une pente, la hauteur d'œil ci-dessous ne vaudrait rien.
    let found = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y, 0)))
        .find(|&at| {
            !maze.has_wall(at, Side::East)
                && flat(at)
                && maze.neighbour(at, Side::East).is_some_and(flat)
        });
    let Some(at) = found else {
        return Vec::new();
    };

    let plane = export::CELL * (at.0 + 1) as f32 - MARGIN;
    let spot = export::ground(maze, at);
    let z = spot[2] + HALF.z + EYE_ABOVE;

    let mut out = Vec::with_capacity(GAPS.len() * 4);
    for gap in GAPS {
        // Depuis la case puis depuis le passage : le même portail, franchi dans
        // un sens et dans l'autre. La caméra neutre regarde le +X, donc le demi-
        // tour est ce qui le remet devant elle.
        for (side, half) in [(-1.0, 0.0), (1.0, core::f32::consts::PI)] {
            for bias in [0.0, core::f32::consts::FRAC_PI_4] {
                out.push(Camera {
                    position: Vec3::new(plane + side * gap, spot[1], z),
                    orientation: Quat::from_axis_angle(
                        Vec3::new(0.0, 0.0, 1.0),
                        Angle::from_radians(half + bias),
                    ),
                    ..Camera::DEFAULT
                });
            }
        }
    }
    out
}

/// Le nombre de pixels où deux images diffèrent, sur le total.
///
/// **Un `assert_eq!` sur les deux tampons ne convient pas** : il recrache deux
/// fois deux cent trente kilooctets en notation de tableau, soit deux mégaoctets
/// de sortie où rien ne se lit. Un décompte dit la seule chose qui informe — la
/// part d'image perdue —, et c'est aussi elle qui distingue les deux manières dont
/// un portail peut manquer.
fn divergence(left: &[u8], right: &[u8]) -> (usize, usize) {
    let differing = left
        .chunks(BYTES_PER_PIXEL)
        .zip(right.chunks(BYTES_PER_PIXEL))
        .filter(|(a, b)| a != b)
        .count();
    (differing, WIDTH as usize * HEIGHT as usize)
}

/// Les deux chemins rendent la même image à chacune des poses, et la rendent
/// complète.
///
/// **C'est le corps commun des oracles de rendu**, extrait à sa troisième
/// occurrence : la traversée d'un côté, le décor entier de l'autre, et le même
/// décompte. Ce qui distingue un oracle du suivant est le choix des poses, et lui
/// seul — c'est ce qu'il fallait rendre évident, puisque l'angle mort d'un oracle
/// est toujours là.
///
/// **Le budget vaut huit fois le décor** : le chemin brut soumet la carte entière,
/// visible ou non, et le défaut du moteur serait de le laisser refuser pour une
/// raison qui n'a rien à voir avec ce qu'on mesure.
fn accord(scenery: &Scenery, poses: &[Camera]) {
    let budget = scenery.map.triangle_count() * 8;

    for (rank, &pose) in poses.iter().enumerate() {
        let cell = scenery.map.locate(pose.position);
        assert_ne!(cell, 0, "la pose {rank} est hors de la carte");

        let mut walked = context(budget);
        let seen = submit(&mut walked, scenery, &View { camera: pose, cell })
            .expect("scène soumise par la traversée");
        assert_eq!(
            seen,
            Visibility::Complete,
            "la traversée tronque à la pose {rank} : les budgets du moteur ne suffisent pas à ce décor"
        );

        let mut whole = context(budget);
        whole.set_camera(pose).expect("caméra posée");
        whole
            .submit_world(Affine3::IDENTITY, &scenery.map, |rank| {
                scenery.materials.get(rank as usize)
            })
            .expect("décor entier soumis");

        let (differing, total) = divergence(&frame(&mut walked), &frame(&mut whole));
        assert_eq!(
            differing, 0,
            "la traversée diverge du décor entier sur {differing} pixels sur {total} \
             à la pose {rank}, œil ({:.2} {:.2} {:.2})",
            pose.position.x, pose.position.y, pose.position.z
        );
    }
}

/// La scène se construit hors de toute fenêtre.
///
/// C'est la contrainte que `ROADMAP.md` annonce pour l'étape 8 : l'image animée
/// du `README` appellera le moteur directement, sans boucle ni fenêtre, et ne
/// pourra le faire que si la construction de la scène vit hors du rappel. Cette
/// épreuve est ce qui le garantit — elle cesserait de compiler si `submit`
/// réclamait un `Tick`, une entrée ou un tampon de sortie.
#[test]
fn la_scene_se_rend_hors_fenetre() {
    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    // La pose du joueur plutôt qu'une pose écrite ici : c'est celle que le jeu
    // emploie vraiment, et elle tient compte de la forme du sol sous ses pieds.
    let player = Player::spawn(&scenery.maze, &scenery.map);

    let mut context = context(0);
    let view = View {
        camera: FreeCamera::new(player.eye()).camera(),
        cell: player.cell(),
    };
    submit(&mut context, &scenery, &view).expect("scène soumise");
    let pixels = frame(&mut context);

    // Une cellule est un volume **fermé** : depuis l'intérieur, chaque pixel
    // tombe sur une de ses surfaces. Un trou dirait qu'une face a disparu au
    // découpage, ou qu'un portail n'a pas été déplié — deux défauts qui ne
    // lèvent aucune erreur et ne se voient qu'à l'image.
    let lit = pixels.chunks(4).filter(|p| p[..3] != [0, 0, 0]).count();
    let total = WIDTH as usize * HEIGHT as usize;
    assert!(
        lit * 100 / total >= 95,
        "l'image n'est peinte qu'à {} %",
        lit * 100 / total
    );
}

/// La traversée rend la **même image** que le décor soumis en entier, et elle la
/// rend complète.
///
/// **C'est l'oracle du rendu, et le moteur le dit ainsi** : la traversée s'ajoute
/// au chemin brut, elle ne le remplace pas, et c'est contre lui qu'elle se valide.
/// L'élimination est conservatrice — ce qu'elle écarte était caché —, donc les
/// deux images doivent coïncider au bit près. Une divergence dirait qu'elle a
/// écarté du visible, ce qu'aucune erreur ne signale.
///
/// **Et ce décor est celui qui l'éprouve enfin.** Les cartes du moteur ont deux à
/// quelques cellules, toutes visibles ; ici il y en a des centaines dont une vue
/// n'atteint que quelques-unes. C'est le cas pour lequel la traversée existe, et
/// il n'avait jamais été joué. Il a fallu pour cela que le budget de triangles
/// devienne réglable depuis un hôte : le chemin brut soumet la carte entière.
#[test]
fn la_traversee_rend_la_meme_image_que_le_decor_entier() {
    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    accord(&scenery, &poses(&scenery));
}

/// Le même oracle, **contre le plan d'un portail**.
///
/// **Il est resté vert pendant qu'une bande du champ se vidait de tout décor**, et
/// c'est son seul défaut : le contrôle est le bon, la pose lui manquait. Un oracle
/// ne voit que ce qu'on lui montre, et le centre d'une case ne montre aucune
/// embrasure de près.
///
/// **Elle est née rouge et c'est elle qui a dit que le correctif tenait** : un
/// portail que le plan proche croisait n'était pas déplié, et la traversée annonçait
/// pourtant une image complète. **Elle reste comme garde contre la régression** —
/// ce qu'un relevé ne peut pas faire, puisqu'il demande quelqu'un qui marche.
#[test]
fn la_traversee_rend_la_meme_image_contre_un_portail() {
    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    let poses = poses_contre_un_portail(&scenery);
    assert!(!poses.is_empty(), "aucune case plate n'ouvre vers l'est");
    accord(&scenery, &poses);
}

/// Les poses d'une marche où l'image s'est amputée, en abscisse, ordonnée, cote
/// et lacet en degrés.
///
/// **Relevées, non construites**, et c'est leur seule raison d'être ici : les
/// trente épisodes d'un parcours du 2026-10-03, sur la carte que [`settings`]
/// engendre — ce sont aussi les réglages du jeu, donc rien n'est à transposer.
///
/// **Le tangage est absent parce qu'il était nul partout**, aux trente épisodes.
/// C'est une mesure et non une simplification : le vol du diagnostic garde
/// l'horizon, et ce qui vide l'image n'a donc rien à lui devoir.
///
/// **La cote est la même partout, et c'est ce qu'elles apportent** : cinq, donc
/// l'étage du haut, là où [`poses_contre_un_portail`] travaille à l'étage du bas.
/// C'était la dernière différence entre une marche qui se trouait et une épreuve
/// qui ne voyait rien.
const WALKED: [(f32, f32, f32, f32); 30] = [
    (32.58, 34.36, 5.00, 166.16),
    (31.59, 34.34, 5.00, 190.99),
    (29.89, 35.42, 5.00, 110.77),
    (29.72, 36.46, 5.00, 91.67),
    (29.63, 39.46, 5.00, 91.67),
    (29.60, 40.46, 5.00, 91.67),
    (28.58, 41.17, 5.00, 169.98),
    (27.59, 41.29, 5.00, 179.53),
    (24.59, 41.32, 5.00, 179.53),
    (23.59, 41.32, 5.00, 179.53),
    (20.59, 41.35, 5.00, 179.53),
    (19.59, 41.36, 5.00, 179.53),
    (16.59, 41.38, 5.00, 179.53),
    (15.59, 41.39, 5.00, 179.53),
    (12.59, 41.42, 5.00, 177.62),
    (11.59, 41.75, 5.00, 147.06),
    (10.30, 43.42, 5.00, 91.67),
    (10.45, 44.39, 5.00, 28.65),
    (11.40, 45.00, 5.00, 43.93),
    (12.40, 45.97, 5.00, 36.29),
    (13.68, 47.42, 5.00, 74.48),
    (13.69, 48.41, 5.00, 103.13),
    (12.57, 49.75, 5.00, 154.70),
    (11.60, 49.92, 5.00, 181.44),
    (10.17, 51.42, 5.00, 85.94),
    (10.54, 52.41, 5.00, 55.39),
    (11.40, 53.13, 5.00, 32.47),
    (12.40, 54.08, 5.00, 61.12),
    (13.07, 55.39, 5.00, 63.03),
    (13.36, 56.40, 5.00, 85.94),
];

/// Le même oracle, **aux poses d'une marche**.
///
/// **Elle est la seule à ne rien choisir**, et c'est pour cela qu'elle vaut : ses
/// poses viennent du jeu, relevées là où l'image s'est amputée, donc elle ne peut
/// pas avoir l'angle mort d'un échantillonnage décidé à l'avance. Les deux autres
/// oracles l'ont eu chacun à leur tour.
///
/// **Les poses se reconstruisent par `FreeCamera`**, comme le jeu les construit, et
/// non par un quaternion écrit ici : c'est ce qui garantit qu'on repose la même
/// orientation sans redeviner l'ordre de composition du lacet et du tangage.
///
/// **Et c'est elle qui a mesuré le découpage du correctif** : quatorze de ces
/// trente poses sont tombées à zéro d'une version du moteur à la suivante, celles
/// dont le lacet est à moins de deux degrés de l'axe du portail ; les seize autres
/// n'avaient pas bougé d'un pixel, dont deux qui perdaient l'écran entier à quatre
/// et dix degrés. La version suivante les a toutes rendues vertes.
#[test]
fn la_traversee_rend_la_meme_image_aux_poses_relevees() {
    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    let poses: Vec<Camera> = WALKED
        .iter()
        .map(|&(x, y, z, yaw)| {
            let at = Vec3::new(x, y, z);
            FreeCamera {
                yaw: yaw.to_radians(),
                pitch: 0.0,
                ..FreeCamera::new(at)
            }
            .camera()
        })
        .collect();

    accord(&scenery, &poses);
}
