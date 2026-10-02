// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le labyrinthe : la grille qu'on perce, et la carte qu'on en tire.
//!
//! La frontière entre les deux est nette, et c'est ce qui rend l'étape testable :
//! `grid` ne connaît que des cases et des murs, sans une seule cote de monde ;
//! l'export qui viendra ensuite traduit une case en cellule et un mur percé en
//! portail, et c'est lui seul qui parle en unités.

pub mod grid;
