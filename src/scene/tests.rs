// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la scène.

use super::*;
use screengine_play::screengine::{BYTES_PER_PIXEL, Config};
use screengine_play::{Affine3, Angle, Context, FreeCamera, Quat, Visibility};

/// Les réglages du labyrinthe d'épreuve.
fn settings() -> Settings {
    Settings {
        extent: (16, 16, 2),
        seed: 0x5EED_1A8E,
        stairs: 6,
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
        let position = Vec3::new(spot[0], spot[1], spot[2] + EYE);
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

/// L'image que rend un contexte, en octets.
fn frame(context: &mut Context) -> Vec<u8> {
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * BYTES_PER_PIXEL];
    context
        .frame_end(&mut pixels, WIDTH)
        .expect("image rendue hors fenêtre");
    pixels
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
    let entrance = scenery.entrance();
    let cell = scenery.map.locate(entrance);

    let mut context = context(0);
    let view = View {
        camera: FreeCamera::new(entrance).camera(),
        cell,
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

        assert_eq!(
            frame(&mut walked),
            frame(&mut whole),
            "la traversée n'a pas rendu la même image que le décor entier à la pose {rank}"
        );
    }
}
