// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le tirage pseudo-aléatoire du jeu : une graine, une suite.
//!
//! **Écrit ici plutôt que pris d'une bibliothèque** : une graine, une suite, rien à
//! configurer, et une dépendance de moins à justifier dans l'audit des licences.
//!
//! **À part du labyrinthe, où il a vécu seul le temps d'un consommateur.** Le semis
//! en tire ses cases, et ce qui décide d'une créature en tirera sa part : laisser le
//! générateur dans le module qui engendre le décor ferait dépendre l'un et l'autre
//! d'une génération de géométrie pour un utilitaire de tirage.
//!
//! **La reproductibilité est la propriété qui compte**, et elle vient de la graine
//! seule : une même graine rejoue une même suite, donc un même labyrinthe rejoue le
//! même semis. Rien ici ne lit l'horloge ni l'entropie du système.

/// Un générateur xorshift64*.
pub struct Rng(u64);

impl Rng {
    /// Un générateur posé sur une graine.
    pub fn new(seed: u64) -> Self {
        // Une graine nulle rend une suite nulle : on la déplace plutôt que de la
        // refuser, l'appelant ayant le droit d'écrire zéro sans le savoir.
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Le prochain mot de la suite.
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Un entier de zéro à `bound` exclu.
    ///
    /// Le reste d'un tirage sur soixante-quatre bits est biaisé, mais d'au plus
    /// `2⁻⁶¹` pour les bornes employées ici — six côtés pour le creusement, et le
    /// compte des cases candidates pour le semis : un rejet serait de la cérémonie
    /// pour un écart qu'aucune partie ne révélerait.
    ///
    /// # Panique
    ///
    /// Si `bound` est nul, comme toute division par zéro. L'appelant tire dans un
    /// ensemble qu'il a construit, donc il en connaît le compte.
    pub fn below(&mut self, bound: u32) -> u32 {
        (self.next() % u64::from(bound)) as u32
    }
}
