// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du rayon contre le décor.
//!
//! **Des prédicats, et non une comparaison de chemins.** L'export éprouve déjà le
//! rayon du moteur contre son oracle brut, et cette égalité ne prouve rien du tir :
//! les deux chemins passent par la même géométrie, donc une formule fausse les rend
//! faux ensemble. Ce qui attrape un défaut ici est une propriété — une cloison
//! arrête, un passage laisse passer, la portée borne.
//!
//! **Elles partent de la pose réelle du joueur**, par `Player::stand`, et non d'une
//! cote composée à la main : c'est de là que le jeu tire, et une origine inventée
//! éprouverait un tir que personne ne fait.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::maze::export;
use crate::maze::grid::{Grid, Settings};
use crate::player::Player;
use crate::test_support::{SEEDS, cases, maze, plain, sides, unit};

/// Ce qui sépare le centre d'une case de la cloison qui la ferme, en unités de
/// monde.
///
/// La moitié du côté intérieur d'une cellule : c'est la distance qu'un rayon tiré
/// du centre vers un côté parcourt avant de rencontrer le mur, quand il y en a un.
const TO_WALL: f32 = export::INNER / 2.0;

/// De quoi absorber la bande de contact du moteur, en unités de monde.
///
/// Un balayage rend une position *juste avant* ce qu'elle touche, donc le point
/// relevé est à un résidu près de la cloison. Un millième du côté intérieur est
/// trois ordres de grandeur au-dessus de ce résidu et reste très loin de la case
/// voisine, qui est ce que les deux épreuves doivent distinguer.
const SLACK: f32 = export::INNER / 1024.0;

/// Une cloison arrête le rayon, et sa normale s'oppose au tir.
///
/// **Les deux ensemble, parce qu'une seule ne suffit pas** : un rayon arrêté dont la
/// normale regarde ailleurs dénonce une face prise à l'envers, et le sens des sommets
/// est précisément ce qui décide de la face vue dans ce projet. La distance borne le
/// cas — ce qui arrête doit être la cloison de cette case, pas une paroi du fond.
#[test]
fn une_cloison_arrete_le_rayon() {
    let mut walled = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                if !grid.has_wall(at, side) {
                    continue;
                }
                walled += 1;

                let ahead = unit(side);
                let shot = fire(&map, player.eye(), ahead, cell);
                assert!(
                    shot.blocked(),
                    "graine {seed:#x}, case {at:?} : le tir vers {side:?} traverse \
                     la cloison depuis {:?}",
                    player.eye()
                );

                let at_point = shot.impact().expect("un tir bloqué à portée a son point");
                let gone = at_point - player.eye();
                assert!(
                    gone.dot(gone).sqrt() <= TO_WALL + SLACK,
                    "graine {seed:#x}, case {at:?} : le tir vers {side:?} ne touche \
                     qu'à {} du départ, soit au-delà de la cloison",
                    gone.dot(gone).sqrt()
                );

                let hit = shot.hit.expect("la cellule de l'œil existe");
                assert!(
                    hit.normal.dot(ahead) < 0.0,
                    "graine {seed:#x}, case {at:?} : la normale {:?} ne s'oppose pas \
                     au tir vers {side:?}",
                    hit.normal
                );
            }
        }
    }

    // Sans ce compte, l'épreuve serait verte sur un décor sans aucun mur.
    assert!(
        walled > 2000,
        "seules {walled} cloisons ont été éprouvées, le corpus n'en est pas un"
    );
}

/// Un passage ouvert laisse le rayon dépasser la case.
///
/// **C'est la propriété qui discrimine**, et l'épreuve précédente ne la donne pas :
/// un rayon qui s'arrêterait à la cloison de toute case, ouverte ou non, passerait
/// la première sans que rien ne le dise. Ce qui est vérifié est donc que le tir va
/// **plus loin** que là où un mur l'aurait arrêté.
#[test]
fn un_passage_laisse_depasser_le_rayon() {
    let mut open = 0;
    let mut touched = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                if grid.has_wall(at, side) {
                    continue;
                }
                // Le voisin doit être plat lui aussi : une cage de l'autre côté
                // présente ses marches au rayon, qui l'arrêtent légitimement bien
                // avant la cloison du fond.
                let Some(next) = grid.neighbour(at, side) else {
                    continue;
                };
                if !plain(&grid, next) {
                    continue;
                }
                open += 1;

                let shot = fire(&map, player.eye(), unit(side), cell);
                // Le point peut manquer si rien n'est touché à portée, ce qui est
                // encore mieux que de dépasser : c'est le passage le plus ouvert.
                if let Some(at_point) = shot.impact() {
                    touched += 1;
                    let gone = at_point - player.eye();
                    assert!(
                        gone.dot(gone).sqrt() > TO_WALL + SLACK,
                        "graine {seed:#x}, case {at:?} : le tir vers {side:?} s'arrête \
                         à {} alors que ce côté est ouvert",
                        gone.dot(gone).sqrt()
                    );
                }
            }
        }
    }

    assert!(
        open > 500,
        "seuls {open} passages ont été éprouvés, le corpus n'en est pas un"
    );
    // **Et le corpus des cas qui affirment quelque chose n'est pas le même** : un
    // tir qui ne touche rien à portée saute l'assertion, donc une portée ramenée à
    // rien rendrait l'épreuve verte et vide. Un couloir de labyrinthe finit par
    // présenter une paroi avant trente-deux unités, et la plupart le font.
    assert!(
        touched > open / 2,
        "seuls {touched} tirs sur {open} ont touché quelque chose, \
         l'épreuve n'affirme presque rien"
    );
}

/// Un tir parti de nulle part ne rencontre rien, et n'interroge pas la carte.
///
/// **Le partage est à nous, et c'est ce qu'elle fige** : le chemin Rust du moteur
/// rend l'absence aussi bien pour une cellule nulle que pour une cellule inconnue,
/// là où sa frontière C en fait deux cas distincts. Zéro veut dire hors de tout
/// volume, et un tir parti de là n'a aucun décor à rencontrer.
#[test]
fn un_tir_hors_de_tout_volume_ne_rencontre_rien() {
    let (_, map) = maze(SEEDS[0]);
    let shot = fire(&map, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 0);

    assert!(shot.hit.is_none());
    assert!(!shot.blocked());
    assert!(shot.impact().is_none());
}

/// Le trajet demandé suit la direction visée, sur la longueur de la portée.
///
/// **Ce qu'elle attrape est dans `fire` et nulle part ailleurs** : une direction
/// inversée, une portée oubliée, un facteur appliqué deux fois. Comparer la longueur
/// à la constante qui l'a produite ne prouverait rien seul — c'est la **colinéarité**
/// qui porte l'épreuve, et elle se vérifie contre la direction visée, qui est une
/// entrée.
///
/// **Et le point touché reste dans la portée.** Un point au-delà dénoncerait une
/// fraction lue sur un autre segment que celui qui a été soumis — exactement le
/// piège que l'étape suivante doit éviter en comparant deux fractions.
#[test]
fn la_portee_borne_le_trajet() {
    let mut touched = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                let ahead = unit(side);
                let shot = fire(&map, player.eye(), ahead, cell);

                // Le trajet est la direction visée, mise à l'échelle de la portée :
                // une direction inversée rendrait un produit scalaire négatif, et un
                // facteur fautif une longueur fausse.
                let span = shot.to - shot.from;
                assert!(
                    (span.dot(ahead) - RANGE).abs() <= RANGE / 1024.0,
                    "graine {seed:#x}, case {at:?} : le trajet vers {side:?} avance \
                     de {} le long de la visée, et non de {RANGE}",
                    span.dot(ahead)
                );

                if let Some(at_point) = shot.impact() {
                    touched += 1;
                    let gone = at_point - shot.from;
                    assert!(
                        gone.dot(gone).sqrt() <= RANGE,
                        "graine {seed:#x}, case {at:?} : le tir vers {side:?} touche \
                         à {}, au-delà de sa portée",
                        gone.dot(gone).sqrt()
                    );
                }
            }
        }
    }

    assert!(
        touched > 2000,
        "seuls {touched} tirs ont touché quelque chose, la borne du point \
         n'est presque jamais éprouvée"
    );
}

/// Un trajet tronqué est bloqué, et ne rend pourtant aucun point à poser.
///
/// **Le contact est fabriqué, et il doit l'être** : la troncature n'est pas
/// atteignable sur un décor que ce jeu produit — voir l'épreuve suivante —, donc la
/// seule façon d'éprouver la clause est de lui donner le contact qu'elle refuse.
/// C'est légitime : la méthode est une fonction d'un contact vers un point, et son
/// entrée est publique.
///
/// **Ce qu'elle fige est le refus, pas le contenu du point.** Que le moteur laisse
/// le point au bout du trajet demandé est une lecture de son code, pas une mesure
/// d'ici : la valeur donnée ci-dessous est celle qu'il a été vu initialiser, et
/// l'épreuve ne prétend pas la vérifier.
#[test]
fn un_trajet_tronque_ne_rend_pas_de_point() {
    let from = Vec3::new(2.0, 2.0, 1.5);
    let to = from + Vec3::new(RANGE, 0.0, 0.0);

    // Ce qu'une troncature rend : la fraction reculée, la surface et la normale
    // effacées — et le point laissé au bout du trajet.
    let shot = Shot {
        from,
        to,
        cell: 1,
        hit: Some(Hit {
            fraction: 0.5,
            normal: Vec3::ZERO,
            point: to,
            surface: 0,
            cell: 1,
            start_solid: false,
            incomplete: true,
            no_gap: false,
        }),
    };

    assert!(
        shot.blocked(),
        "un trajet tronqué arrête le tir, la réponse étant conservatrice"
    );
    assert!(
        shot.impact().is_none(),
        "le point d'un trajet tronqué est au-delà de ce qui a été examiné, \
         donc il ne se pose pas"
    );
}

/// Un rayon qui vise le centre d'un volume y entre, et à la bonne fraction.
///
/// **La fraction est ce qui se vérifie, pas le fait d'entrer** : c'est elle qui se
/// comparera à celle du décor, et une fraction juste à un facteur près passerait un
/// test de présence sans que le départage soit jamais correct.
#[test]
fn un_rayon_qui_vise_le_centre_entre_dans_le_volume() {
    let from = Vec3::ZERO;
    let to = Vec3::new(10.0, 0.0, 0.0);
    let volume = Volume {
        centre: Vec3::new(5.0, 0.0, 0.0),
        half: Vec3::new(1.0, 1.0, 1.0),
    };

    // La face d'entrée est à quatre unités sur un trajet de dix.
    let at = enters(from, to, &volume).expect("le volume est sur le trajet");
    assert!(
        (at - 0.4).abs() <= f32::EPSILON * 8.0,
        "l'entrée est à {at} et non à 0,4"
    );
}

/// Le seuil de la touche est la demi-étendue, et il se joue des deux côtés.
///
/// **Un seul des deux cas ne prouverait rien** : un test qui touche toujours passe
/// le premier, un test qui ne touche jamais passe le second. C'est l'encadrement qui
/// dit que le volume a la taille annoncée.
#[test]
fn le_seuil_de_la_touche_est_la_demi_etendue() {
    let volume = Volume {
        centre: Vec3::new(5.0, 0.0, 0.0),
        half: Vec3::new(1.0, 1.0, 1.0),
    };
    let margin = 1.0 / 1024.0;

    // Décalé en `y` de moins que la demi-étendue : le rayon passe dedans.
    let inside = Vec3::new(0.0, volume.half.y - margin, 0.0);
    assert!(
        enters(inside, inside + Vec3::new(10.0, 0.0, 0.0), &volume).is_some(),
        "un rayon à {} du centre manque un volume de {}",
        volume.half.y - margin,
        volume.half.y
    );

    // Et de plus : il passe à côté.
    let outside = Vec3::new(0.0, volume.half.y + margin, 0.0);
    assert!(
        enters(outside, outside + Vec3::new(10.0, 0.0, 0.0), &volume).is_none(),
        "un rayon à {} du centre touche un volume de {}",
        volume.half.y + margin,
        volume.half.y
    );
}

/// Un volume derrière le départ n'est pas touché, et un volume au-delà du bout non plus.
///
/// **Les deux bornes du segment, et c'est ce que l'intersection avec `[0, 1]` tient.**
/// Sans la borne basse, tirer vers l'avant toucherait ce qui est dans le dos ; sans la
/// borne haute, la portée ne voudrait rien dire.
#[test]
fn un_volume_hors_du_segment_n_est_pas_touche() {
    let from = Vec3::ZERO;
    let to = Vec3::new(10.0, 0.0, 0.0);
    let half = Vec3::new(1.0, 1.0, 1.0);

    let behind = Volume {
        centre: Vec3::new(-5.0, 0.0, 0.0),
        half,
    };
    assert!(
        enters(from, to, &behind).is_none(),
        "un volume dans le dos est touché par un tir vers l'avant"
    );

    let beyond = Volume {
        centre: Vec3::new(15.0, 0.0, 0.0),
        half,
    };
    assert!(
        enters(from, to, &beyond).is_none(),
        "un volume au-delà du bout du trajet est touché"
    );
}

/// Un rayon parallèle à deux faces décide, et un départ sur une face aussi.
///
/// **Elle n'éprouve pas le bras parallèle de `slab`**, et c'est mesuré : débrancher ce
/// bras ne fait bouger aucune épreuve, la division par une composante nulle rendant
/// `±∞` — exact — et les `NaN` du cas tangent étant absorbés par `f32::max`. Ce
/// qu'elle éprouve est le **résultat** dans les trois poses où un axe ne bouge pas,
/// qui sont le cas courant d'un tir : un rayon horizontal ne bouge ni en `z`, et un
/// rayon axial sur aucun des deux autres axes.
///
/// **La troisième pose est la seule qui peut produire un `NaN`** : un départ
/// exactement sur le plan d'une face, parallèlement à elle. Elle rase le volume, donc
/// elle le touche — et c'est ce qui se vérifie.
#[test]
fn un_rayon_parallele_a_une_face_decide() {
    let volume = Volume {
        centre: Vec3::new(5.0, 0.0, 0.0),
        half: Vec3::new(1.0, 1.0, 1.0),
    };

    // Parallèle en `y` et en `z`, et dedans sur ces deux axes : il entre.
    let along = enters(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), &volume);
    assert!(
        along.is_some_and(|at| !at.is_nan()),
        "un rayon axial manque le volume, ou en rend une entrée qui est un NaN : \
         {along:?}"
    );

    // Parallèle en `y` et en `z`, mais dehors en `z` : il ne peut pas y entrer,
    // quelle que soit la distance parcourue en `x`.
    let past = enters(Vec3::new(0.0, 0.0, 2.0), Vec3::new(10.0, 0.0, 2.0), &volume);
    assert!(
        past.is_none(),
        "un rayon parallèle qui passe au-dessus touche quand même, à {past:?}"
    );

    // Exactement sur la face basse, et parallèle à elle : il la rase, donc il entre.
    let grazing = Vec3::new(0.0, 0.0, volume.centre.z - volume.half.z);
    let flush = enters(grazing, grazing + Vec3::new(10.0, 0.0, 0.0), &volume);
    assert!(
        flush.is_some_and(|at| !at.is_nan()),
        "un rayon qui rase la face basse rend {flush:?}"
    );
}

/// Un tir à bout portant dans un volume le touche, à la fraction zéro.
///
/// **Ce n'est pas un cas limite mais la réponse juste** : le segment est déjà dedans,
/// donc il y entre à l'instant où il part. Lue autrement, une créature collée à l'œil
/// deviendrait intouchable — exactement quand il faut la toucher.
#[test]
fn un_depart_dans_le_volume_touche_a_zero() {
    let volume = Volume {
        centre: Vec3::ZERO,
        half: Vec3::new(1.0, 1.0, 1.0),
    };
    let at = enters(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), &volume);
    assert_eq!(at, Some(0.0));
}

/// À fraction égale, le décor gagne — et d'un cheveu, le volume.
///
/// **La règle isolée de toute géométrie**, parce que c'est la seule façon de la poser
/// au bit : placer sur un décor engendré un volume dont la face tombe pile sur le plan
/// d'un mur n'est pas quelque chose qu'un flottant garantisse.
#[test]
fn a_fraction_egale_le_decor_gagne() {
    /// Un volume de ce rang atteint à cette fraction.
    fn reach(rank: usize, at: f32) -> Option<Reach> {
        Some(Reach { rank, at })
    }

    assert_eq!(arbitrate(reach(0, 0.5), Some(0.5)), Struck::Decor);
    assert_eq!(arbitrate(reach(0, 0.5), Some(0.500_001)), Struck::Volume(0));
    assert_eq!(arbitrate(reach(0, 0.500_001), Some(0.5)), Struck::Decor);

    // Et ce que chacun donne seul.
    assert_eq!(arbitrate(reach(2, 0.9), None), Struck::Volume(2));
    assert_eq!(arbitrate(None, Some(0.9)), Struck::Decor);
    assert_eq!(arbitrate(None, None), Struck::Nothing);
}

/// Un mur protège le volume qui est derrière, et pas celui qui est devant.
///
/// **Le prédicat central du lot, et il part du décor réel** : c'est lui qui dit que le
/// tir ne traverse pas les murs, ce qu'aucune épreuve de géométrie pure ne peut dire —
/// le test de tranches ne connaît rien du décor et déclare touché un démon derrière une
/// paroi.
///
/// **Les deux poses sont les mêmes à une unité près**, de part et d'autre de la
/// cloison : c'est ce qui en fait un encadrement et non deux affirmations.
#[test]
fn un_mur_protege_le_volume_qui_est_derriere() {
    let mut walled = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                if !grid.has_wall(at, side) {
                    continue;
                }
                walled += 1;
                let ahead = unit(side);

                // Devant la cloison : à mi-chemin de l'œil au mur.
                let near = Volume {
                    centre: player.eye() + ahead * (TO_WALL / 2.0),
                    half: crate::monster::HALF,
                };
                assert_eq!(
                    resolve(&map, player.eye(), ahead, cell, &[near]).struck,
                    Struck::Volume(0),
                    "graine {seed:#x}, case {at:?} : un volume devant la cloison \
                     de {side:?} n'est pas touché"
                );

                // Derrière : d'une unité au-delà du mur, donc hors de la case.
                let far = Volume {
                    centre: player.eye() + ahead * (TO_WALL + 1.0),
                    half: crate::monster::HALF,
                };
                assert_eq!(
                    resolve(&map, player.eye(), ahead, cell, &[far]).struck,
                    Struck::Decor,
                    "graine {seed:#x}, case {at:?} : un volume derrière la cloison \
                     de {side:?} est touché à travers elle"
                );
            }
        }
    }

    assert!(
        walled > 2000,
        "seules {walled} cloisons ont été éprouvées, le corpus n'en est pas un"
    );
}

/// Ce que le décor oppose, et les deux cas où il n'oppose rien.
///
/// **Le départ dans le solide est le piège que le lot existe pour éviter** : le moteur
/// rend alors une fraction nulle, et lue comme un obstacle elle ferait gagner le décor
/// contre tout volume — plus rien ne serait jamais touchable, et le symptôme serait un
/// tir qui cesse de porter sans qu'on sache pourquoi.
///
/// **Les contacts sont fabriqués, et il est mesuré qu'ils doivent l'être** : aucun
/// départ pris dans l'épaisseur d'une cloison ne lève ce statut sur les six graines, un
/// rayon n'ayant aucune dilatation et l'épaisseur d'un mur n'appartenant à aucune
/// cellule d'un décor fermé. Le cas reste publié au contrat, donc la clause tient.
#[test]
fn un_depart_dans_le_solide_n_oppose_aucun_obstacle() {
    /// Un contact de décor à cette fraction, avec ou sans départ dans le solide.
    fn touch(fraction: f32, start_solid: bool) -> Option<Hit> {
        Some(Hit {
            fraction,
            normal: Vec3::new(-1.0, 0.0, 0.0),
            point: Vec3::ZERO,
            surface: 7,
            cell: 1,
            start_solid,
            incomplete: false,
            no_gap: false,
        })
    }

    assert_eq!(
        obstacle(touch(0.5, false)),
        Some(0.5),
        "un mur fait obstacle"
    );
    assert_eq!(
        obstacle(touch(0.0, true)),
        None,
        "un départ dans le solide fait obstacle à distance nulle"
    );
    assert_eq!(
        obstacle(touch(1.0, false)),
        None,
        "un trajet libre fait obstacle"
    );
    assert_eq!(
        obstacle(None),
        None,
        "un tir parti de nulle part en fait un"
    );

    // Et la troncature en fait un, elle : le trajet a été borné sans être examiné.
    let mut cut = touch(0.5, false).expect("contact fabriqué");
    cut.incomplete = true;
    cut.surface = 0;
    cut.normal = Vec3::ZERO;
    assert_eq!(
        obstacle(Some(cut)),
        Some(0.5),
        "une troncature laisse passer un volume situé au-delà"
    );
}

/// Le plus proche des volumes l'emporte, quel que soit son rang.
///
/// **L'ordre de la tranche ne doit rien décider**, et c'est ce que le second cas
/// vérifie : les deux mêmes volumes rangés à l'envers rendent le même vainqueur, par
/// un rang différent. Un `min_by` remplacé par un premier trouvé passerait le premier
/// cas seul.
#[test]
fn le_plus_proche_des_volumes_l_emporte() {
    let (grid, map) = maze(SEEDS[0]);
    let player = Player::stand(&grid, &map, grid.start());
    let cell = player.eye_cell(&map);
    let half = crate::monster::HALF;

    for side in sides() {
        let ahead = unit(side);
        let close = Volume {
            centre: player.eye() + ahead * (TO_WALL / 4.0),
            half,
        };
        let further = Volume {
            centre: player.eye() + ahead * (TO_WALL / 2.0),
            half,
        };

        assert_eq!(
            resolve(&map, player.eye(), ahead, cell, &[close, further]).struck,
            Struck::Volume(0),
            "vers {side:?} : le plus proche rangé en tête ne gagne pas"
        );
        assert_eq!(
            resolve(&map, player.eye(), ahead, cell, &[further, close]).struck,
            Struck::Volume(1),
            "vers {side:?} : le plus proche rangé en second ne gagne pas"
        );
    }
}

/// La borne de cellules du balayage est hors d'atteinte d'un rayon axial.
///
/// **C'est une mesure, et elle explique l'épreuve précédente.** Le format d'export
/// plafonne une coordonnée de texture à `MAX_TEXEL_COORD` texels, à raison de cent
/// vingt-huit texels par unité de monde : le décor ne peut donc pas s'étendre au-delà
/// de cent vingt-huit unités, soit **trente-deux cases**. Or la borne du balayage se
/// consomme en **cellules**, et il en faut soixante-cinq dégagées pour la saturer :
/// un rayon axial n'en rencontre jamais assez, et une enfilade qui les porterait ne
/// se charge pas.
///
/// Elle part du décor **le plus long qui se charge** pour que ce soit le format qui
/// donne le chiffre, et non une constante recopiée ici.
#[test]
fn la_borne_du_balayage_est_hors_de_portee_du_decor() {
    // Une enfilade d'une case de large n'a qu'un chemin — la ligne entière —, donc
    // c'est le décor qui aligne le plus de cellules pour une étendue donnée.
    let length = 32;
    let grid = Grid::generate(Settings {
        extent: (length, 1, 1),
        seed: SEEDS[0],
        stairs: 0,
        ramps: 0,
        loops: 0,
    });
    let map = World::load(&export::world(&grid)).expect("enfilade engendrée valide");
    assert!(
        (length as usize) < screengine_play::SWEEP_CELLS,
        "une enfilade de {length} cases porte assez de cellules pour saturer \
         la borne de {}, et la troncature cesse d'être hors d'atteinte",
        screengine_play::SWEEP_CELLS
    );

    let player = Player::stand(&grid, &map, (0, 0, 0));
    let cell = player.eye_cell(&map);
    assert_ne!(cell, 0, "le joueur naît dans l'enfilade");

    // Jusqu'au-delà du bout : le rayon parcourt l'enfilade entière sans que la
    // région examinée soit bornée.
    let to = player.eye() + Vec3::new(1.0, 0.0, 0.0) * (length as f32 * export::CELL);
    let hit = map
        .pick(cell, player.eye(), to, Surfaces::Solid)
        .expect("la cellule de départ existe");
    assert!(
        !hit.incomplete,
        "le décor le plus long qui se charge tronque déjà un rayon, \
         à la fraction {}",
        hit.fraction
    );
}
