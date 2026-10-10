// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pose l'icône du jeu dans l'exécutable Windows.
//!
//! **Une ressource Win32 et non un réglage de fenêtre**, parce que l'étage
//! d'accueil du moteur n'en expose pas : `Play` règle le titre, la résolution,
//! les tuiles, les deux budgets, la cadence et la sortie par Échap, et rien pour
//! l'icône. Le jeu n'ayant aucun accès à la fenêtre, il ne peut pas la poser
//! lui-même.
//!
//! **Ce que la ressource rattrape, et ce qu'elle ne rattrape pas** : sous
//! Windows, une fenêtre sans icône explicite prend celle de son exécutable, donc
//! la barre des tâches et l'explorateur montrent la bonne — c'est un
//! contournement complet pour cette cible. Sous X11 et Wayland il n'y a pas
//! d'équivalent : l'icône y vient du fichier `.desktop` qui accompagne
//! l'installation, jamais du binaire.
//!
//! **Rien ne s'exécute hors de Windows.** Le script est compilé pour toutes les
//! cibles — cargo l'exige —, mais la dépendance qui écrit la ressource n'est
//! déclarée que pour elles, et ce fichier se réduit alors à sa condition.

fn main() {
    // Le chemin est relatif à la racine du paquet, que cargo donne comme
    // répertoire courant du script.
    println!("cargo:rerun-if-changed=assets/icons/dedale.ico");

    #[cfg(windows)]
    {
        let mut ressource = winresource::WindowsResource::new();
        ressource.set_icon("assets/icons/dedale.ico");
        // Un échec ici arrêterait la compilation sur une cible où l'icône n'est
        // qu'un agrément : le jeu tourne sans elle, et l'avertissement dit
        // pourquoi elle manque plutôt que de refuser de construire.
        if let Err(faute) = ressource.compile() {
            println!("cargo:warning=icône non posée dans l'exécutable : {faute}");
        }
    }
}
