// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la scène.

use super::*;
use screengine_play::FreeCamera;
use screengine_play::screengine::{BYTES_PER_PIXEL, Config, Context};

/// Les réglages du labyrinthe d'épreuve.
fn settings() -> Settings {
    Settings {
        extent: (16, 16, 2),
        seed: 0x5EED_1A8E,
        stairs: 6,
        loops: 8,
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
    const WIDTH: u32 = 640;
    const HEIGHT: u32 = 360;

    let scenery = Scenery::new(settings()).expect("labyrinthe et planches valides");
    let entrance = scenery.entrance();
    let cell = scenery.map.locate(entrance);

    let mut context = Context::new(Config {
        max_width: WIDTH,
        max_height: HEIGHT,
        width: WIDTH,
        height: HEIGHT,
        tile_size: 64,
        max_triangles: 0,
        max_lines: 0,
    })
    .expect("configuration tenable");

    let view = View {
        camera: FreeCamera::new(entrance).camera(),
        cell,
    };
    submit(&mut context, &scenery, &view).expect("scène soumise");

    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * BYTES_PER_PIXEL];
    context
        .frame_end(&mut pixels, WIDTH)
        .expect("image rendue hors fenêtre");

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
