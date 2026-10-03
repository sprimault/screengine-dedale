# Tout passe par ce fichier, jamais par `cargo` en direct : une commande tapée à
# la main perd les réglages de `makefile.local` — répertoire de sortie, chaîne
# d'outils, variables temporaires — et l'écart ne se voit pas dans la sortie.
#
# `makefile.local` est local et hors dépôt. Un clone sans lui compile dans
# `target/` à la racine, ce qui marche : la redirection est une contrainte de
# poste, pas du projet.
-include makefile.local

SORTIE ?= .tmp

.PHONY: aide build run release test attentes fmt lint audit deny clean

aide:
	@echo "make build    compile le jeu, profil dev"
	@echo "make run      lance le jeu dans une fenêtre"
	@echo "make release  compile en release"
	@echo "make test     les tests du jeu"
	@echo "make attentes les épreuves en attente d'un correctif du moteur"
	@echo "make fmt      cargo fmt --check"
	@echo "make lint     clippy -D warnings"
	@echo "make audit    les avis de sécurité, interrogés en direct"
	@echo "make deny     licences, sources, doublons"

build:
	cargo build

# Le jeu ouvre une fenêtre : il ne fait partie d'aucun contrôle automatique, et
# c'est en le lançant qu'on juge ce qu'aucun test ne décrit.
run:
	cargo run

release:
	cargo build --release

test:
	cargo test

# Les épreuves qu'un défaut du moteur fait échouer, écrites avant son correctif
# et marquées `#[ignore = "…"]` avec la raison. `make test` les saute — c'est pour
# cela qu'elles sont marquées —, donc elles ne se relancent que par ici, et une
# attente qu'on ne relance jamais est la dette que le marqueur devait éviter.
attentes:
	cargo test -- --ignored

fmt:
	cargo fmt --check

# `-D warnings` sur les tests aussi : un avertissement dans un test est un test
# qui ne dit pas ce qu'il croit.
lint:
	cargo clippy --all-targets -- -D warnings

# Interroge sa base d'avis **en direct** : un contrôle vert le matin peut être
# rouge l'après-midi sur exactement le même code.
audit:
	cargo audit

deny:
	cargo deny check

clean:
	cargo clean
