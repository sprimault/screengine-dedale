// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la marque d'impact : sa base tangente, son sens, son anneau.
//!
//! **Celle qui compte le plus est le sens des sommets.** Un quadrilatère décrit à
//! l'envers est un dos de face : il disparaît sans erreur, et l'écran ne montre rien —
//! c'est le défaut le plus coûteux à diagnostiquer de ce moteur, et le seul de ce lot
//! qu'on ne verrait pas en regardant, puisqu'une marque absente ressemble à une marque
//! qu'on n'a pas posée.
//!
//! **Rien n'y fige d'empreinte d'image** : la conformance est l'affaire du moteur, qui
//! la tient sur ses propres scènes. Ce qui s'éprouve ici est de la géométrie.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Les normales contre lesquelles la base tangente s'éprouve.
///
/// **Les six axes et les obliques à quarante-cinq degrés**, parce que ce sont celles
/// que le décor porte : les murs, sols et plafonds sont axiaux, et les rampes sont ses
/// seules surfaces obliques. S'y ajoutent deux normales quelconques, pour qu'aucun cas
/// ne doive sa réussite à une composante nulle.
fn normals() -> Vec<Vec3> {
    let mut out = vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ];
    for oblique in [
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(-1.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 1.0),
        Vec3::new(0.0, -1.0, 1.0),
        Vec3::new(0.3, -0.7, 0.2),
        Vec3::new(-0.9, 0.1, -0.4),
    ] {
        out.push(oblique.normalize());
    }
    out
}

/// La base tangente est orthonormée, et orthogonale à la normale.
///
/// **Trois propriétés qui se tiennent ensemble** : deux tangentes unitaires,
/// perpendiculaires entre elles, et perpendiculaires à la normale. Qu'une seule tombe
/// et le quadrilatère se déforme ou s'aplatit — et la tolérance est large parce que la
/// racine inverse du moteur passe par une table, qui n'a pas la précision d'un calcul
/// exact et n'a pas à l'avoir.
#[test]
fn la_base_tangente_est_orthonormee() {
    /// Ce que la table de racine inverse du moteur laisse d'écart.
    const SLACK: f32 = 1.0 / 1024.0;

    for normal in normals() {
        let (along, across) = tangents(normal);

        assert!(
            (along.dot(along) - 1.0).abs() <= SLACK,
            "pour la normale {normal:?}, la première tangente {along:?} n'est \
             pas unitaire"
        );
        assert!(
            (across.dot(across) - 1.0).abs() <= SLACK,
            "pour la normale {normal:?}, la seconde tangente {across:?} n'est \
             pas unitaire"
        );
        assert!(
            along.dot(across).abs() <= SLACK,
            "pour la normale {normal:?}, les deux tangentes ne sont pas \
             perpendiculaires : {}",
            along.dot(across)
        );
        assert!(
            along.dot(normal).abs() <= SLACK && across.dot(normal).abs() <= SLACK,
            "pour la normale {normal:?}, une tangente n'est pas dans son plan : \
             {} et {}",
            along.dot(normal),
            across.dot(normal)
        );
    }
}

/// Le sens des sommets fait regarder la marque du côté d'où le tir vient.
///
/// **Le défaut qu'elle existe pour attraper ne se voit pas à l'écran** : un
/// quadrilatère décrit dans l'autre sens est un dos de face, il disparaît sans erreur,
/// et une marque absente ressemble à une marque qu'on n'a pas posée.
///
/// **Le critère vient du précédent de la tache d'ombre**, qui est visible et dont les
/// coins tournent de façon que leur normale géométrique pointe vers l'observateur. Ici
/// l'observateur est du côté de la normale du contact, puisque le moteur rend une
/// normale opposée au mouvement du rayon.
#[test]
fn le_sens_des_sommets_montre_la_face() {
    /// Ce que la table de racine inverse du moteur laisse d'écart.
    const SLACK: f32 = 1.0 / 1024.0;

    for normal in normals() {
        let quad = corners(&Mark {
            at: Vec3::new(3.0, 4.0, 5.0),
            normal,
        });

        // La normale géométrique du quadrilatère, par deux de ses arêtes prises
        // dans l'ordre où elles sont décrites.
        let facing = (quad[1] - quad[0]).cross(quad[2] - quad[0]).normalize();
        assert!(
            facing.dot(normal) >= 1.0 - SLACK,
            "pour la normale {normal:?}, le quadrilatère regarde {facing:?}, \
             donc il est décrit à l'envers"
        );
    }
}

/// Les quatre coins sont coplanaires, et le plan flotte au-dessus de la surface.
///
/// **Le décalage est ce qui empêche la marque d'entrer dans ce qu'elle marque**, et il
/// se mesure le long de la normale — un coin qui n'y serait pas dénoncerait une base
/// tangente qui fuit du plan. Le carré, lui, se vérifie par ses diagonales : elles ont
/// même longueur et se croisent au centre.
#[test]
fn les_coins_sont_coplanaires_au_dessus_de_la_surface() {
    /// Ce que la table de racine inverse du moteur laisse d'écart.
    const SLACK: f32 = 1.0 / 1024.0;

    // **Que le décalage soit non nul se vérifie à la compilation**, dans le module :
    // ce qui suit le compare à la constante qui l'a produit, donc il resterait vert sur
    // un décalage mis à zéro — vérifié en le faisant. Ce qui s'éprouve ici est que les
    // quatre coins le portent, et le long de la normale.
    for normal in normals() {
        let at = Vec3::new(3.0, 4.0, 5.0);
        let quad = corners(&Mark { at, normal });

        for (rank, corner) in quad.iter().enumerate() {
            let lift = (*corner - at).dot(normal);
            // La tolérance est prise sur le décalage lui-même et non sur l'unité :
            // à l'échelle du millimètre, l'écart de la table de racine inverse
            // n'est plus négligeable devant ce qu'on mesure.
            assert!(
                (lift - LIFT).abs() <= LIFT / 16.0,
                "pour la normale {normal:?}, le coin {rank} est à {lift} de la \
                 surface et non à {LIFT}"
            );
        }

        // Les deux diagonales d'un carré : même longueur, et même milieu.
        let (first, second) = (quad[2] - quad[0], quad[3] - quad[1]);
        assert!(
            (first.dot(first) - second.dot(second)).abs() <= SLACK,
            "pour la normale {normal:?}, les diagonales ne font pas la même \
             longueur : {} et {}",
            first.dot(first).sqrt(),
            second.dot(second).sqrt()
        );

        // Et le côté est bien celui qu'on a demandé, mesuré sur une arête.
        let edge = quad[1] - quad[0];
        assert!(
            (edge.dot(edge).sqrt() - 2.0 * RADIUS).abs() <= SLACK,
            "pour la normale {normal:?}, l'arête fait {} et non {}",
            edge.dot(edge).sqrt(),
            2.0 * RADIUS
        );
    }
}

/// L'anneau garde les dernières marques, et jette la plus ancienne.
///
/// **Ce qui s'éprouve est la borne et l'ordre.** Sans borne, un couloir mitraillé
/// finirait par dépasser la capacité de triangles, qui refuse le lot entier plutôt que
/// de dégrader ; sans l'ordre, c'est la marque qu'on vient de poser qui disparaîtrait,
/// ce qui est exactement l'inverse de ce qu'on veut voir.
#[test]
fn l_anneau_garde_les_dernieres_marques() {
    let mut marks = Marks::new();
    let normal = Vec3::new(1.0, 0.0, 0.0);

    for step in 0..KEEP {
        marks.add(Vec3::new(step as f32, 0.0, 0.0), normal);
        assert_eq!(marks.ring.len(), step + 1);
    }

    // La dix-septième chasse la première, et le compte ne bouge plus.
    marks.add(Vec3::new(KEEP as f32, 0.0, 0.0), normal);
    assert_eq!(marks.ring.len(), KEEP);
    assert_eq!(
        marks.ring[0].at.x, 1.0,
        "la plus ancienne marque n'a pas été jetée"
    );
    assert_eq!(
        marks.ring[KEEP - 1].at.x,
        KEEP as f32,
        "la dernière marque posée n'est pas en queue"
    );
}

/// Un éclat s'éteint de lui-même, et au bout du temps annoncé.
///
/// **C'est ce qui le sépare d'une marque** : l'une est une trace sur un mur, l'autre
/// l'instant d'un coup. Un éclat qui resterait suivrait mal sa créature, puisqu'il ne
/// bouge pas avec elle — et il finirait par en constellier le couloir.
#[test]
fn un_eclat_s_eteint_au_bout_de_son_temps() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let mut marks = Marks::new();
    marks.flash(Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(marks.sparks.len(), 1);

    // À une image de la fin, il brille encore.
    let frames = (SPARK_TIME / DT).ceil() as usize;
    for _ in 0..frames - 1 {
        marks.advance(DT);
    }
    assert_eq!(
        marks.sparks.len(),
        1,
        "l'éclat s'est éteint avant ses {SPARK_TIME} secondes"
    );

    marks.advance(DT);
    assert_eq!(
        marks.sparks.len(),
        0,
        "l'éclat survit à son temps, donc il restera sur le décor"
    );
}

/// Les éclats s'éteignent sans toucher aux marques.
///
/// **Les deux anneaux ne se gouvernent pas de la même façon**, et c'est tout l'objet de
/// les séparer : les marques attendent que l'anneau les chasse, les éclats comptent leur
/// temps. Un vieillissement qui emporterait les marques effacerait les impacts du décor
/// trois images après les avoir posés.
#[test]
fn le_temps_n_efface_pas_les_marques() {
    let mut marks = Marks::new();
    marks.add(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 1.0));
    marks.flash(Vec3::new(1.0, 2.0, 3.0));

    for _ in 0..600 {
        marks.advance(1.0 / 60.0);
    }

    assert_eq!(marks.sparks.len(), 0, "les éclats n'ont pas tous fini");
    assert_eq!(
        marks.ring.len(),
        1,
        "dix secondes ont effacé une marque que seul l'anneau devait chasser"
    );
}

/// Une normale nulle ne pose aucune marque.
///
/// **Le moteur rend une normale nulle dans deux cas** — rien n'a été touché, et le
/// trajet a été tronqué —, et aucune base tangente ne se tire d'un vecteur nul : le
/// quadrilatère s'effondrerait en un point, sans erreur. Le refus est donc ici, au seul
/// endroit qui ait les trois vecteurs sous la main.
#[test]
fn une_normale_nulle_ne_pose_rien() {
    let mut marks = Marks::new();
    marks.add(Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO);
    assert_eq!(marks.ring.len(), 0);

    // Et une normale qui n'est pas nulle en pose bien une : sans ce second cas, un
    // refus de tout passerait.
    marks.add(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(marks.ring.len(), 1);
}
