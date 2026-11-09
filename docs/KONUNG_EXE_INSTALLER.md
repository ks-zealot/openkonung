# Konung CD installer, uninstaller and payload

Targets on the original CD (`originalCD\`):

| file | size | SHA-256 |
|---|---|---|
| `konung.exe` | 482 304 | `178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e` |
| `inst\Install.exe` | 562 688 | `3a7f3fd23e9975b280b48755a0111d784f5401475681c267f79cc7b143bb5dd6` |
| `Uninstall.exe` (payload / install root) | 432 128 | `581848ff91f81ed65c9d1fe189d1ace9eb36a9206ebb1af146bc9c09f3606d1e` |
| `Demo\demo.exe` | 435 712 | `bf170dc86270e2873c7aae1b32587c47559df22f4251bb1f2b0b3e62db7246eb` |

`konung.exe` is the **launcher**; `Install.exe` is the **installer**;
`Uninstall.exe` is copied into the install root by the installer. All three are
Borland Delphi applications and share the VCL/`TBo*`/`VCLUnZip` framework.

## 1. Installer entry points and UI data

`inst\Install.exe` imports the same VCL/registry/shell units and contains the
literals:

- `Install.ini`, `ListFile.ini`, `\Install.dat`, `\Install.zip`,
  `\Setup.dat`, `\Setup.zip`, `\Uninstall.exe`;
- `VCLUnZip`, `kpZipObj`, `TZipHeaderInfo`, `TIncompleteZip`;
- `\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\`,
  `UninstallString`, `UninstallKey`, `UninstallName`;
- shortcut keys `GROUP`, `DESKTOP`, `Desktop`, `WorkingDirectory`,
  `IconLocation`, `IconIndex`;
- `Setup_Run` (the window class used by the launcher too).

`inst\Install.dat` (1 200 124 bytes) is a **streaming ZIP** with 11 entries:

| entry | compressed | raw |
|---|---|---|
| `00.bmp` … `07.bmp` | ~51–125 KB each | 231 478 each |
| `Install.img` | 375 798 | 520 500 |
| `Install.ini` | 827 | 1 848 |
| `Install.str` | 514 | 685 |

`00.bmp` is the installer background; `01..07.bmp` are the progress/screen
replacements (`[replace_Image] 10=01.bmp … 70=07.bmp`); `Install.img` is the
installer image list/bitmap; `Install.str` is a Delphi `TStringsCollection`
with the finish/parameter/readme memo texts.

## 2. `Install.ini` — installer configuration

```ini
[Button]
next=Продолжить >
close=Закрыть
setup=Установить >
preview=< Вернуться
break=Прервать

[replace_Image]
10=01.bmp … 70=07.bmp

[screen]
Image=00.bmp
640_480=0,0;262,125
800_600=0,0;422,243
1024_768=175,75;605,371

[Caption]
Title=Князь
Caption=Установка игры "Князь: Легенды Лесной страны"
Caption_Begin=…
Caption_Finish=…
Caption_Look_Parameter=…
Caption_Progress=…
Caption_readme=…
Caption_SetFolderGrup=…
Caption_SetFolderProgram=…
Copy_to=Копируется файл
result_setup=Результат установки

[Shortcut]
1=Uninstall_1
2=Konung_1
3=Konung_2

[Uninstall_1]
LinkName=<GROUP>\Удалить.lnk
FileName=<ROOT>\Uninstall.exe
WorkingDirectory=<ROOT>

[Konung_1]
LinkName=<Desktop>\Князь.lnk
FileName=<CDROM>\Konung.exe
WorkingDirectory=<CDROM>
IconLocation=<ROOT>\Konung.ico
IconIndex=0

[Konung_2]
LinkName=<GROUP>\Князь.lnk
FileName=<CDROM>\Konung.exe
WorkingDirectory=<CDROM>
IconLocation=<ROOT>\Konung.ico
IconIndex=0

[Install]
UninstallKey=Konung: Legend of the North
UninstallName=Князь: Легенды Лесной страны
Version=1.0
RegRoot1C=\SOFTWARE\1C
RegRoot=\Software\1C\Konung
NameGrup=1C\Князь
NameFolder=1C\Konung

[message]
Break_setup_q=Прервать установку?
Break_setup=Установка прервана
exit_q=Выйти из программы установки?
in_drive=На диске
not_memory=:\ недостаточно места
error_copy1=Неизвестная ошибка копирования
error_copy2=Ошибка копирования файла
not_drive_local=Диск не является локальным
```

Shortcut targets point to `<CDROM>\Konung.exe`, not to the installed game:
the shortcut starts the **launcher**, which then finds the installed
`Root\Konung.res` through the registry. This is why the launcher must remain on
the CD (or be copied alongside) for the shortcut to work.

## 3. `ListFile.ini` — install payload list

`inst\setup.dat` contains `ListFile.ini` (1 337 bytes) listing **119 files**:

- executables/libraries: `CLIENT.EXE`, `konung.res`, `PLAYAVI.DLL`,
  `Uninstall.exe`, `konung.ico`;
- game data: `Game.0`, `Game.1`, `Game.2`, `Game.res`, `Objects.res`,
  `Sounds_.res`, `Sounds_.res`/`SOUNDS.RES`, `Quest_r.res`/`QUESTR.RES`,
  `FLAMES.RES`, `Maps.res`, `LIGHTS.RES`;
- all `Konung.1..50`, `LIGHTS.1..50`;
- interface archives: `INTO/INTR/ME/MENU/NEW/NEWH/PLAY/SAVE` for 640 and 800;
- `Uninstall.ini`.

`inst\setup.dat` is a **streaming ZIP** (75 543 320 bytes) with 119 local
entries and no central directory; raw payload total is **142 835 552 bytes**.
The two data-descriptor entries (`Objects.res`, `konung.res`, flags `0x0002`)
still parse from their local headers.

Key payload sizes:

| entry | compressed | raw |
|---|---|---|
| `Objects.res` | 31 878 800 | 52 522 058 |
| `SOUNDS.RES` | 16 090 816 | 21 981 240 |
| `GRAPH.RES` | 8 168 673 | 14 505 371 |
| `CLIENT.EXE` | 152 106 | 939 008 |
| `konung.res` | 178 085 | 1 586 688 |
| `Game.0/1/2` | ~30 KB each | 709 822 each |
| `Game.res` | 9 362 | 365 509 |
| `Maps.res` | 108 593 | 3 856 000 |
| `Quest_r.res` | 122 888 | 337 800 |
| `Questr.RES` | 8 637 | 337 800 |
| `playavi.dll` | 167 845 | 316 416 |
| `Uninstall.exe` | 224 312 | 432 128 |

## 4. Install flow

1. The launcher's **Установка игры** button runs `<CD>\inst\Install.exe`.
2. `Install.exe` reads `Install.ini` (UI/captions/shortcuts/registry) and
   `ListFile.ini` (file list).
3. It opens `Setup.dat` (or `Setup.zip`) and `Install.dat` (or `Install.zip`)
   through `VCLUnZip`/`kpZipObj`.
4. It asks for the destination folder (default `1C\Konung` under Program Files),
   checks free space, and extracts the 119 files with a progress bar
   (`Install.img`/`00..07.bmp`, `Copy_to` status).
5. It copies `Uninstall.exe`, `Uninstall.ini` and `ListFile.ini` into the
   install root.
6. It writes the registry:
   - `HKLM\Software\1C\Konung` with `Root` (install dir), `Version` (`1.0`),
     and the uninstall data;
   - `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Konung: Legend of
     the North` with `UninstallString`, `DisplayName`, etc.
7. It creates the shortcuts from `[Shortcut]`:
   - `<Desktop>\Князь.lnk` and `<GROUP>\Князь.lnk` → `<CDROM>\Konung.exe`;
   - `<GROUP>\Удалить.lnk` → `<ROOT>\Uninstall.exe`.
8. Finish page (`Install.str`: "Установка игры ... успешно завершена").

The launcher's installed check reads the same `Root`/`Version` values and
requires `Root\Client.exe` to exist. The game itself is launched as
`Root\Konung.res`.

## 5. Uninstaller

`Uninstall.exe` (copied from `setup.dat`) reads `Uninstall.ini` and
`ListFile.ini`:

```ini
[uses_save]
1=konung.sa*
2=konung.cfg
3=client.log
4=clans.cfg

[button]
button_close=Выход
button_delete=Удалить
button_breakdown=Отказаться

[Caption]
Title=Удаление игры Князь
...
[Install]
UninstallKey=Konung: Legend of the North
Version=1.0
RegRoot1C=\SOFTWARE\1C
RegRoot=\Software\1C\Konung
[Shortcut]
1=<GROUP>\Удалить.lnk
2=<Desktop>\Князь.lnk
3=<GROUP>\Князь.lnk
```

- It removes the files listed by `ListFile.ini` from the install root.
- It asks whether to delete the save files (`konung.sa*`, `konung.cfg`,
  `client.log`, `clans.cfg`); declining preserves them.
- It removes the shortcuts and the uninstall registry key.

## 6. Demo launcher

`Demo\demo.exe` is a generic 1C demo installer/launcher. It bundles zlib
(`inflate 1.0.4`) and is not part of the Konung install; the CD's
`Demo\allods3\DEMO.AVI` and `Demo\WAR` are separate demo payloads. The Konung
launcher's **Демо** button runs `Demo\Demo.exe`.

## 7. Verified vs unresolved

**Verified**

- `Konung.dat`, `Install.dat`, `setup.dat` are streaming ZIPs (no EOCD).
- `setup.dat` contains the full game payload, 119 entries, including
  `Client.exe`, `konung.res`, `Game.0/1/2`, all maps/lights, `Objects.res`,
  `Sounds_.res`, `Quest_r.res`, `Uninstall.exe`.
- `Install.ini`, `ListFile.ini`, `Uninstall.ini` contents.
- Installer writes `Root`/`Version` and the uninstall key; launcher reads
  `Root`/`Version` and checks `Root\Client.exe`.
- Shortcuts point to `<CDROM>\Konung.exe`, not the installed game.

**Unresolved**

1. Exact `Install.exe` extraction loop and progress-state code (not decompiled
   line-by-line; the UI/data contract is verified).
2. Exact registry value names written by `Install.exe` beyond `Root`,
   `Version`, `UninstallString`, `DisplayName`.
3. Whether `Install.exe` prefers `Setup.zip`/`Install.zip` over the `.dat`
   variants in the same way the launcher prefers `konung.zip`.
4. `Install.img`/`Install.str` internal layout (UI only).
