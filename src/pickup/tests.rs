// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du motif qui répartit les deux sortes.
//!
//! **C'est la seule arithmétique du module**, et c'est voulu : la pose passe par
//! [`Body::stand`], qui a ses propres épreuves auprès du corps — dont celle qui dit
//! qu'une créature naît posée sur son sol, sur six graines et quelle que soit la forme
//! du sol. La redire ici mesurerait deux fois la même fonction, ce que `jeu.md` écarte.
//!
//! **Ce qui reste et qui n'est mesuré nulle part ailleurs** : que le motif rende la
//! proportion annoncée, et qu'il ne groupe pas les sortes. Le reste du module est du
//! dessin et de la soumission, donc se regarde.
//!
//! **Le compte multiple du motif, lui, est tenu à la compilation**, par une assertion
//! dans le module : c'est une relation entre deux constantes, et une épreuve ne la
//! verrait qu'après que le binaire ait été produit.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use screengine_play::FreeCamera;

use super::*;
use crate::player;
use crate::test_support::{context, frame, maze};

/// Les deux lots de sprites sont acceptés par le moteur.
///
/// **Ce qu'elle garde est ce qui a déjà échoué deux fois.** Un lot **vide** est refusé
/// depuis la version épinglée, donc un labyrinthe qui ne porterait qu'une sorte ferait
/// disparaître les deux ; et une coordonnée de texture hors borne refuse le lot entier.
/// Les deux se voient ici, là où à l'écran un objet manquant ne dit pas pourquoi.
///
/// **Ce qu'elle ne mesure pas, et il faut le savoir** : que l'objet couvre des pixels.
/// Une image rendue sans décor reste noire sur ce chemin, faute de maîtriser la séquence
/// du rendu hors fenêtre — c'est l'étape 8 qui l'ouvrira pour l'image animée. Le défaut
/// qui a motivé cette épreuve était d'ailleurs invisible à toute assertion : des
/// coordonnées écrites de zéro à un prenaient **un texel**, celui du coin transparent, et
/// le moteur l'acceptait sans broncher.
///
/// **L'œil se place depuis la pose de l'objet et non l'inverse** : la pose vient du
/// balayage, donc elle dépend de la forme du sol de la case — l'écrire ici obligerait à
/// la deviner, et une épreuve qui devine une cote la devine fausse le jour où la cote
/// bouge.
#[test]
fn les_deux_lots_sont_acceptes() {
    let (grid, map) = maze(0x1);
    // Une case par sorte : les deux lots doivent être non vides, le moteur refusant une
    // soumission sans ressource depuis la version épinglée.
    let start = grid.start();
    let spots = [start, (start.0, start.1 + 1, start.2)];
    let pickups = Pickups::scatter(&spots, &grid, &map).expect("les vignettes se décodent");
    let item = &pickups.items()[0];

    let eye = Vec3::new(item.at.x - 2.0, item.at.y, item.at.z);
    let mut frame_context = context(64);
    frame_context
        .set_camera(FreeCamera::new(eye).camera())
        .expect("caméra tenable");
    assert!(
        submit(&mut frame_context, &pickups).is_ok(),
        "le moteur refuse un lot d'objets posés devant l'œil"
    );
    // L'image se termine quand même : un contexte laissé en cours de soumission ferait
    // échouer l'épreuve suivante qui en demande un, le relevé de tuiles étant global.
    let _ = frame(&mut frame_context);
}

/// Un volume posé sur un objet le touche, et le retire.
///
/// **Le contact se mesure sur la pose que le balayage a rendue**, pas sur le centre de la
/// case : un objet posé sur une marche n'est pas à la hauteur de son voisin, et un test
/// écrit sur la case le prendrait à travers un plancher.
///
/// **Et le volume du joueur, pas celui de l'objet** : les deux gabarits diffèrent, donc un
/// recouvrement écrit sur une seule demi-étendue serait faux des deux côtés. C'est la même
/// clause que pour la morsure d'une créature.
#[test]
fn un_volume_pose_dessus_touche_et_retire() {
    let (grid, map) = maze(0x1);
    let start = grid.start();
    let spots = [start, (start.0, start.1 + 1, start.2)];
    let mut pickups = Pickups::scatter(&spots, &grid, &map).expect("les vignettes se décodent");

    let at = pickups.items()[0].at;
    let (rank, kind) = pickups
        .touching(at, player::HALF)
        .expect("un volume posé sur l'objet ne le touche pas");
    assert_eq!(rank, 0, "le rang touché n'est pas celui de l'objet");
    assert_eq!(kind, Kind::Box, "la sorte touchée n'est pas celle du rang");

    pickups.remove(rank);
    assert_eq!(pickups.items().len(), 1, "l'objet retiré est encore là");
    assert!(
        pickups.touching(at, player::HALF).is_none(),
        "l'objet retiré touche encore"
    );
}

/// Un volume à l'écart ne touche rien.
///
/// **Sans elle, un test qui répondrait toujours vrai passerait** : l'épreuve ci-dessus ne
/// dit que le cas qui touche, et c'est le couple des deux qui mesure le recouvrement.
///
/// **L'écart est pris sur les deux gabarits**, donc il ne peut pas être trop faible : la
/// somme des demi-étendues est exactement la limite du contact, et une unité au-delà la
/// dépasse quel que soit le réglage des deux.
#[test]
fn un_volume_a_l_ecart_ne_touche_rien() {
    let (grid, map) = maze(0x1);
    let start = grid.start();
    let pickups = Pickups::scatter(&[start], &grid, &map).expect("les vignettes se décodent");

    let at = pickups.items()[0].at;
    let clear = HALF.x + player::HALF.x + 1.0;
    let aside = Vec3::new(at.x + clear, at.y, at.z);

    assert!(
        pickups.touching(aside, player::HALF).is_none(),
        "un volume à {clear} unités de l'objet le touche"
    );
}

/// La sorte qu'un rang reçoit, comme la pose la choisit.
fn kind_at(rank: usize) -> Kind {
    PATTERN[rank % PATTERN.len()]
}

/// Le motif rend douze boîtes et huit fioles sur un labyrinthe.
///
/// **Les deux chiffres sont écrits en dur, et c'est voulu** : ce sont des cotes de jeu,
/// pas une conséquence qu'on recalcule. Les dériver de [`SPOTS`] et de [`PATTERN`]
/// rendrait l'épreuve verte quelle que soit la valeur des deux, donc muette le jour où
/// l'une bouge — alors que c'est précisément le moment où la rareté doit se rejuger.
///
/// **Et elle a déjà payé** : le compte est passé de dix à vingt après une traversée sans
/// croiser un seul objet, et c'est cette épreuve qui a rougi pour le dire.
#[test]
fn le_motif_donne_douze_boites_et_huit_fioles() {
    let boxes = (0..SPOTS).filter(|&r| kind_at(r) == Kind::Box).count();
    let vials = (0..SPOTS).filter(|&r| kind_at(r) == Kind::Vial).count();

    assert_eq!(boxes, 12, "le motif rend {boxes} boîtes et non douze");
    assert_eq!(vials, 8, "le motif rend {vials} fioles et non huit");
    assert_eq!(boxes + vials, SPOTS, "des rangs ne reçoivent rien");
}

/// Les deux sortes se mêlent, et aucune ne se groupe.
///
/// **C'est tout l'objet d'un motif plutôt qu'une coupure**, et le symptôme qu'il évite
/// est lisible : le semis répartit ses cases par étage, donc une coupure aurait posé les
/// munitions à un niveau et les fioles à l'autre. Ce qui est vérifié est qu'aucune fenêtre
/// de la longueur du motif ne porte une seule sorte.
///
/// **Sur deux tours de motif et non un** : le raccord entre deux tours est le seul
/// endroit où deux rangs de même sorte se suivent, et une épreuve d'un seul tour ne le
/// verrait pas.
#[test]
fn aucune_sorte_ne_se_groupe() {
    let span = PATTERN.len();

    for start in 0..=span {
        let window: Vec<Kind> = (start..start + span).map(kind_at).collect();
        assert!(
            window.contains(&Kind::Box) && window.contains(&Kind::Vial),
            "les rangs {start} à {} ne portent qu'une sorte",
            start + span - 1
        );
    }
}
