// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pose l'icône du jeu dans l'exécutable Windows.
//!
//! **Une ressource Win32 en plus du réglage de fenêtre, et non à sa place** :
//! elle est ce que l'explorateur et la barre des tâches lisent, qui regardent le
//! fichier et jamais la fenêtre. Le jeu pose la sienne par l'étage d'accueil, et
//! les deux chemins ne desservent pas le même endroit.
//!
//! **Hors de Windows elle ne sert rien** : aucun bureau n'y tire une icône d'un
//! exécutable. Sous X11 c'est la fenêtre qui la porte, sous Wayland le fichier
//! `.desktop` que l'installation pose.
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
