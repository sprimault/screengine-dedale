// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la scène.

use super::*;
use crate::maze::grid::Side;
use crate::player::{EYE_ABOVE, HALF, Player};
use screengine_play::screengine::{BYTES_PER_PIXEL, Config};
use screengine_play::{Affine3, Angle, Context, FreeCamera, Quat, Vec3, Visibility};

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

/// La largeur des images d'épreuve.
const WIDTH: u32 = 320;

/// Leur hauteur.
const HEIGHT: u32 = 180;

/// Un contexte hors fenêtre, au budget de triangles donné.
///
/// `0` prend le défaut du moteur, qui suffit à la traversée mais **pas** au
/// chemin brut : celui-ci soumet toutes les cellules de la carte, visibles ou
/// non, et les compte toutes.
fn context(max_triangles: u32) -> Context {
    Context::new(Config {
        max_width: WIDTH,
        max_height: HEIGHT,
        width: WIDTH,
        height: HEIGHT,
        tile_size: 64,
        max_triangles,
        max_lines: 0,
    })
    .expect("configuration tenable")
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

/// L'image que rend un contexte, en octets.
fn frame(context: &mut Context) -> Vec<u8> {
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * BYTES_PER_PIXEL];
    context
        .frame_end(&mut pixels, WIDTH)
        .expect("image rendue hors fenêtre");
    pixels
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
    let budget = scenery.map.triangle_count() * 8;

    for (rank, pose) in poses(&scenery).into_iter().enumerate() {
        let cell = scenery.map.locate(pose.position);
        assert_ne!(cell, 0, "la pose {rank} est hors de la carte");

        let mut walked = context(budget);
        let seen = submit(&mut walked, &scenery, &View { camera: pose, cell })
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
             à la pose {rank}"
        );
    }
}

/// Le même oracle, **contre le plan d'un portail**.
///
/// **Il est resté vert pendant qu'une bande du champ se vidait de tout décor**, et
/// c'est son seul défaut : le contrôle est le bon, la pose lui manquait. Un oracle
/// ne voit que ce qu'on lui montre, et le centre d'une case ne montre aucune
/// embrasure de près.
///
/// **En attente nommée, et c'est elle qui dira que le correctif tient** : un
/// portail que le plan proche croise n'est pas déplié par la traversée, qui annonce
/// pourtant une image complète. **Elle reste ensuite**, comme garde contre la
/// régression — ce qu'un relevé ne peut pas faire, puisqu'il demande quelqu'un qui
/// marche.
#[test]
#[ignore = "un portail que le plan proche croise n'est pas déplié, et la traversée l'annonce complète"]
fn la_traversee_rend_la_meme_image_contre_un_portail() {
    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    let budget = scenery.map.triangle_count() * 8;

    let poses = poses_contre_un_portail(&scenery);
    assert!(!poses.is_empty(), "aucune case plate n'ouvre vers l'est");

    for (rank, pose) in poses.into_iter().enumerate() {
        let cell = scenery.map.locate(pose.position);
        assert_ne!(cell, 0, "la pose {rank} est hors de la carte");

        let mut walked = context(budget);
        let seen = submit(&mut walked, &scenery, &View { camera: pose, cell })
            .expect("scène soumise par la traversée");
        assert_eq!(
            seen,
            Visibility::Complete,
            "la traversée tronque à la pose {rank}"
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
             à la pose {rank}, dont l'œil est en x = {}",
            pose.position.x
        );
    }
}
