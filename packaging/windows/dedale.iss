; L'installateur Windows, pour Inno Setup 6.
;
; **Ce fichier commence par un BOM, et ce n'est pas un accident.** Inno Setup lit
; un `.iss` en UTF-8 à condition d'y trouver cette marque ; sans elle il
; l'interprète dans la page de codes du système, et « Dédale » devient du
; charabia dans le nom du programme et dans le menu Démarrer. `.gitattributes`
; déclare l'extension pour qu'aucune normalisation ne la retire.
;
; Ce qu'il apporte et ce qu'il n'apporte pas : un raccourci, une désinstallation
; propre et une entrée dans la liste des programmes. Il ne lève pas
; l'avertissement du système, qui frappe un installateur non signé autant qu'un
; exécutable nu — rien n'est signé ici, et `docs/construction.md` le dit.
;
; Le binaire est autonome : les planches, les textures et la police sont dedans.
; L'installateur ne pose donc qu'un fichier, et son intérêt est entièrement dans
; ce que le système en sait, pas dans ce qu'il copie.
;
; La version et le chemin du binaire viennent de la ligne de commande, que le
; workflow remplit : les écrire ici en ferait une seconde source, qui finirait
; par ne plus s'accorder avec `Cargo.toml`.

; **Deux racines, et les confondre est le défaut que la relecture a attrapé** :
; `Charge` est le répertoire de mise en scène, qui ne porte que ce qui
; s'installe — le binaire et les textes —, tandis que `Depot` est l'arbre des
; sources, d'où viennent l'icône du programme d'installation et la licence qu'il
; affiche. Rien n'oblige les deux à coïncider, et ils ne coïncident pas.
#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Binaire
  #define Binaire "screengine-dedale.exe"
#endif
#ifndef Charge
  #define Charge ".."
#endif
#ifndef Depot
  #define Depot ".."
#endif

[Setup]
AppId={{8E2F4C11-6B3D-4A7E-9C25-5D1A3F8B0E47}
AppName=Dédale
AppVersion={#Version}
AppPublisher=Stéphane Primault
AppPublisherURL=https://github.com/sprimault/screengine-dedale
DefaultDirName={autopf}\Dedale
DefaultGroupName=Dédale
; Sans privilèges : le jeu s'installe pour l'utilisateur courant, ce qui évite
; l'élévation. Un jeu autonome n'a rien à écrire ailleurs que chez lui.
PrivilegesRequired=lowest
OutputBaseFilename=dedale-{#Version}-windows-x86_64-installateur
SetupIconFile={#Depot}\assets\icons\dedale.ico
UninstallDisplayIcon={app}\{#Binaire}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile={#Depot}\LICENSE-MIT
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "francais"; MessagesFile: "compiler:Languages\French.isl"
Name: "anglais"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#Charge}\{#Binaire}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Charge}\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "{#Charge}\README.fr.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Charge}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Charge}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Charge}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Charge}\THIRD-PARTY-NOTICES"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Dédale"; Filename: "{app}\{#Binaire}"
Name: "{autodesktop}\Dédale"; Filename: "{app}\{#Binaire}"; Tasks: bureau

[Tasks]
Name: "bureau"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Run]
Filename: "{app}\{#Binaire}"; Description: "{cm:LaunchProgram,Dédale}"; Flags: nowait postinstall skipifsilent
