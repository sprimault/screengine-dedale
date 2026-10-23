# Security

Français : [SECURITY.fr.md](SECURITY.fr.md)

## Supported versions

The latest published version. There is no maintenance branch: a fix ships in the
next version, never as a backport onto an earlier one.

## Reporting a vulnerability

Through a private security advisory on the GitHub repository, never a public
issue. Answer within a few days.

## What is in scope

**The game reads nothing it did not write itself, and that is what draws its
scope.** The executable is self-contained: sprite sheets, textures and the font
go into it at build time, the maze is generated from a seed, and nothing is
looked up at startup — no configuration file, no command-line argument, no
environment variable. There is therefore no entry through which hostile content
would arrive, and that is deliberate.

What remains, and what counts:

- an archive or installer attached to a release whose contents do not match what
  the repository holds at the tag;
- a security advisory affecting one of the game's dependencies, if the
  repository's own checks have not already raised it;
- a defect in the game that would read, write or execute something outside what
  it states it does, or that would give loaded content a way to run — **nothing
  that goes into the binary allows this**: these are blocks of texels and a font,
  which the game copies without ever interpreting them.

## What is not

**The rendering engine has its own channel.** `screengine` is a dependency pinned
by commit, and it is the one decoding bytes: a flaw in its loading of textures,
meshes or maps is reported in its repository, which carries the matching scope.
Only what the game hands it belongs here.

**Archives are not signed**, on Windows or anywhere else, and
`docs/construction.md` says so: a downloaded executable therefore triggers a
system warning. That is a known and accepted limit, not a vulnerability.

Modifying your own files to get a different game. Nothing here is to be protected
from its owner.
