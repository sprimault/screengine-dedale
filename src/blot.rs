// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le disque modulant qu'on engendre : dense au centre, blanc au bord.
//!
//! **Deux surfaces du jeu veulent exactement cette image** — la tache d'ombre sous
//! une créature, et la marque d'impact d'un tir —, à son côté et à sa densité près.
//! Elle vit donc ici plutôt que dans l'un des deux modules, qui n'ont aucune raison
//! de dépendre l'un de l'autre.
//!
//! **Blanc au bord et non transparent, et c'est ce qui décide de tout** : `255` est le
//! neutre de la modulation, donc un texel blanc laisse le tampon intact. Le disque
//! s'éteint ainsi de lui-même sur son pourtour, là où la transparence binaire du
//! moteur donnerait un bord franc — qui se lirait comme un disque posé.
//!
//! **Engendré et non chargé** : c'est un dégradé radial, il n'a aucun détail à porter,
//! et son filtrage fait le reste. Un asset pour cela coûterait une planche à produire
//! et à relever, pour une image qu'une boucle écrit.

use screengine_play::Texture;

#[cfg(test)]
mod tests;

/// Un disque modulant de ce côté, à cette densité en son centre.
///
/// `side` est un côté en texels, et **le moteur exige une puissance de deux** ; `core`
/// est ce que le centre laisse passer sur 255, donc d'autant plus sombre qu'il est
/// petit. Le bord vaut toujours 255, qui ne module rien.
///
/// **La courbe est quadratique parce que c'est la courbe voulue**, et non pour épargner
/// une racine : une tache dense sous le corps et qui s'efface vite. Le carré du rayon
/// normalisé la donne directement, donc la racine n'aurait rien à faire ici.
pub fn blot(side: u32, core: f32) -> Texture {
    let mut bytes = Vec::with_capacity((side * side) as usize * 4);
    let half = side as f32 / 2.0;

    for v in 0..side {
        for u in 0..side {
            let (dx, dy) = (u as f32 + 0.5 - half, v as f32 + 0.5 - half);
            let fade = ((dx * dx + dy * dy) / (half * half)).min(1.0);
            let level = (core + (255.0 - core) * fade) as u8;
            bytes.extend_from_slice(&[level, level, level, 0xFF]);
        }
    }

    Texture::load(side, side, &bytes)
        .unwrap_or_else(|_| unreachable!("carrée, puissance de deux, et de la bonne longueur"))
}

/// Un disque **clair et découpé**, du cœur au bord, à transparence binaire.
///
/// **L'inverse du précédent, et pour l'inverse des raisons** : une surface modulée ne
/// peut qu'assombrir, donc elle ne sert pas un éclat qui doit se détacher d'une
/// silhouette sombre. Celui-ci se peint, donc il éclaircit — au prix d'un bord franc,
/// la transparence du moteur étant binaire, et c'est ce que le dégradé de couleur fait
/// oublier à cette taille.
///
/// **La clarté porte la lisibilité, la teinte ne fait que la colorer.** C'est ce qui le
/// rend indépendant du décor : l'habillage de ce labyrinthe changera, et un éclat accordé
/// à la teinte des murs d'aujourd'hui serait à refaire.
///
/// `core` et `edge` sont les deux couleurs interpolées du centre vers le rayon ; au-delà,
/// le texel est transparent, donc ni peint ni inscrit dans la profondeur.
pub fn spark(side: u32, core: [u8; 3], edge: [u8; 3]) -> Texture {
    let mut bytes = Vec::with_capacity((side * side) as usize * 4);
    let half = side as f32 / 2.0;

    for v in 0..side {
        for u in 0..side {
            let (dx, dy) = (u as f32 + 0.5 - half, v as f32 + 0.5 - half);
            // La racine sert ici, là où le disque modulant s'en passe : la couleur
            // s'interpole sur le **rayon** et non sur son carré, sinon l'orange
            // n'occuperait qu'un anneau mince contre le bord.
            let fade = ((dx * dx + dy * dy).sqrt() / half).min(1.0);
            let blend = |from: u8, to: u8| {
                (f32::from(from) + (f32::from(to) - f32::from(from)) * fade) as u8
            };

            bytes.extend_from_slice(&[
                blend(core[0], edge[0]),
                blend(core[1], edge[1]),
                blend(core[2], edge[2]),
                // Hors du disque inscrit dans le carré, rien n'est peint : c'est la
                // découpe, et le seuil du moteur est à 128.
                match fade < 1.0 {
                    true => 0xFF,
                    false => 0x00,
                },
            ]);
        }
    }

    Texture::load_masked(side, side, &bytes)
        .unwrap_or_else(|_| unreachable!("carrée, puissance de deux, et de la bonne longueur"))
}
