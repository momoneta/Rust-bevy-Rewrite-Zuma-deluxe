# Zuma Deluxe — rewrite en Rust + Bevy

Reimplementación del motor de **Zuma Deluxe** (PopCap Games, 2003) en Rust con
[Bevy 0.20](https://bevy.org). El juego lee los **archivos de datos originales**
de tu copia de Zuma Deluxe (imágenes, niveles, curvas, fuentes y sonidos) y
reproduce la jugabilidad a partir del análisis de la descompilación
[`momoneta/zume`](https://github.com/momoneta/zume) y del propio `Zuma.exe`.

> **Aviso legal.** Zuma Deluxe es © PopCap Games / Electronic Arts. Este
> repositorio **no incluye** ningún recurso del juego (ni imágenes, ni sonidos,
> ni niveles, ni el ejecutable). Solo contiene código nuevo escrito en Rust.
> Para jugar necesitas tu propia copia del juego original.

Las capturas no se incluyen porque muestran arte con copyright; puedes generarlas
con tus datos (ver “Capturas” abajo).

## Cómo jugar

1. Instala Rust (1.97.1 o superior, lo exige Bevy 0.20): <https://rustup.rs>
2. En Linux instala las dependencias de Bevy:
   `sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`
3. Copia la carpeta de tu juego (la que contiene `images/`, `levels/`,
   `sounds/`, `fonts/`, `properties/`) junto al proyecto con el nombre `data`:

   ```
   Rust-bevy-Rewrite-Zuma-deluxe/
   ├── Cargo.toml
   ├── src/
   └── data/            <- copia de "Zuma Deluxe" (ignorada por git)
       ├── images/
       ├── levels/
       ├── sounds/
       ├── fonts/
       ├── music/
       └── properties/
   ```

   También puedes indicar la ruta: `cargo run --release -- --data "/ruta/a/Zuma Deluxe"`
   o con la variable de entorno `ZUMA_DATA`.
4. `cargo run --release`

### Controles

| Acción | Control |
|---|---|
| Apuntar | ratón |
| Disparar | clic izquierdo |
| Intercambiar bola actual/siguiente | clic derecho o `Espacio` |
| Menú / pausa | botón **MENU** o `Esc` |

### Música

La música original es `music/zuma.mo3`, un módulo tracker comprimido de BASS
que Bevy no puede decodificar. El script `tools/convert_music.sh` renderiza sus
8 subcanciones a `music/zuma_0.ogg` … `zuma_7.ogg` (necesita `openmpt123` y
`oggenc`: `sudo apt install openmpt123 vorbis-tools`):

```
tools/convert_music.sh data
```

El juego elige la pista igual que el original (que salta a posiciones de orden
del módulo, `FUN_00511100`): tema principal en menús y niveles, música de
peligro cuando la cadena se acerca al agujero, y los *jingles* de nivel
completado, vida perdida, etapa completada y game over. Sin estos archivos el
juego funciona igual, solo sin música. Los efectos de sonido (`sounds/*.ogg`)
funcionan directamente.

## Qué está implementado

**Formatos originales (todo leído de los archivos del juego):**

- `levels/*/*.dat` — curvas `CURV` (versión 1–3): cabecera, datos del editor y
  puntos codificados en deltas de 1/100 px con banderas *en túnel* y
  *prioridad de dibujo*. Formato verificado con el cargador original
  (`FUN_004dfbf0`) y probado con las 25 curvas del juego.
- `levels/levels.xml` — `Graphics`, `Settings`, `LevelProgression`, `Level`,
  `StageProgression`, `Cutout`, `TreasurePoint`… con un parser XML tolerante
  como el de PopCap (atributos sin comillas, duplicados, espacios en `=`).
  Valores por defecto sacados de los constructores del binario
  (`speed=0.5`, `start=40`, `danger=600`, `slowfactor=4`, `zumaback=300`…).
- `properties/resources.xml` — manifiesto del `ResourceManager` (grupos,
  `SetDefaults`, `alphaimage`, `alphagrid`, `rows`/`cols`, alias).
- Imágenes con las reglas alfa de PopCap: `foo.jpg` + `_foo.gif`, `foo_.gif`,
  imágenes solo-alfa, rejillas alfa compartidas y transparencia GIF; búsqueda
  de archivos sin distinguir mayúsculas.
- `fonts/*.txt` — fuentes `ImageFont` (intérprete del lenguaje de
  descripción: `Define`, capas, anchos, offsets, kerning, `SetCharMap`, capas
  con *tags* como `Shadow1`/`Outline`, `LayerSetColorMult`/`ColorAdd`).
- `sounds/*.ogg`.

**Jugabilidad (modelada sobre `Board`, `CurveMgr`, `Ball`, `Bullet`, `Gun`):**

- Cadenas de bolas a 100 ticks/s por curva, entrada rápida inicial (`start`),
  ralentización cerca del agujero (`slowdistance`/`slowfactor`), aviso de peligro.
- Inserción de disparos con animación, empuje de la cadena, grupos de 3+.
- Segmentos que retroceden atraídos por el mismo color (`mergespeed`) y
  reacciones en cadena (combos).
- Puntuación original: `10·bolas + 100·combo + bonus de hueco`;
  CHAIN BONUS `(n+5)·10` desde 5 aciertos seguidos; GAP/DOUBLE/TRIPLE GAP BONUS
  `((500·(300−g))/3000)·10`; monedas `max(500, faltante/600·100)`.
- Power-ups: bomba, ralentizar, marcha atrás y precisión (con guía de disparo).
- Inserción suave: la bola disparada crece y abre la cadena poco a poco.
- Calavera del agujero que se abre al acercarse la cadena (`skullrot`).
- Mezcla aditiva (como `DRAWMODE_ADDITIVE`) para explosiones, chispas, brillos
  y luces, con un `Material2d` propio.
- Monedas en los `TreasurePoint` con la frecuencia `treasurefreq` y duración de 10 s.
- Barra Zuma (`score`): al llenarse deja de entrar bolas y la cadena retrocede.
- Rana con las medidas originales (bola a 25 px de la boca, gema de la
  siguiente bola, capa superior recortada, parpadeo, lengua).
- Túneles y puentes: bolas de prioridad 0 bajo los *cutouts*, ocultas dentro del túnel.
- Vidas, vida extra cada 50.000 puntos, tiempo ACE con bonus, game over.
- HUD original (marco, vidas, nombre del nivel, puntuación MONODIGIT, barra, botón MENU).

**Pantallas:** carga con barra y “CLICK HERE TO PLAY!”, menú principal (cielo
animado, ojos de la rana que siguen al cursor, sol), Aventura (los cuatro
templos y las 12 puertas con las coordenadas originales), Gauntlet (Practice /
Survival, rangos Rabbit/Eagle/Jaguar/Sun God), opciones (volumen, pantalla
completa), diálogos de pausa, nivel completado, vida perdida, game over y
final feliz. El progreso se guarda en `~/.config/zuma-bevy/profile.txt`
(`%APPDATA%\zuma-bevy` en Windows).

### Diferencias conocidas con el original

- La música necesita convertir `zuma.mo3` una vez (ver arriba).
- El número de colores por nivel no está en `levels.xml`; se usa una
  aproximación (4 colores en las etapas 1–2, 5 en la 3–6, 6 a partir de la 7).
- No se reproducen todavía: las piezas animadas de terremoto
  (`BackgroundAlpha`), la animación de los templos en la Aventura, los
  perfiles de usuario múltiples y la tabla de récords con nombres.
- Algunas duraciones (power-ups, retroceso tras la barra Zuma) son estimadas.

## Estructura del código

```
src/
  main.rs            App de Bevy, ventana 640x480 lógica escalada
  data/              lectores de formatos originales (sin dependencia de Bevy)
    vfs.rs           carpeta de datos sin distinguir mayúsculas
    xml.rs           XML tolerante estilo Sexy::XMLParser
    manifest.rs      properties/resources.xml
    image_load.rs    reglas alfa de PopCap
    font.rs          ImageFont (.txt)
    curve.rs         curvas CURV (.dat)
    levels.rs        levels.xml
    validate.rs      test que comprueba todos los datos reales
  assets.rs          carga incremental a texturas/sonidos de Bevy
  gfx.rs             dibujo inmediato (como Sexy::Graphics) sobre sprites de Bevy
  additive.rs        material 2D con mezcla aditiva
  game/              jugabilidad: board.rs, chain.rs, gun.rs, draw.rs
  screens/           estados: carga, menú, aventura, gauntlet, juego, opciones
  ui.rs              botones, diálogos y sliders con el arte original
  audio.rs           efectos y música por pistas
  input.rs profile.rs debug.rs
tools/convert_music.sh   convierte zuma.mo3 a ogg
```

## Pruebas y herramientas de depuración

```
cargo test                      # tests unitarios; con data/ presente valida TODOS los datos
cargo run -- --level 3-5        # empieza directamente en el nivel 3-5
cargo run -- --screen gauntlet  # salta a una pantalla (menu, adventure, gauntlet)
cargo run -- --level 1-1 --autoplay   # un bot apunta y dispara solo
```

### Capturas

```
cargo run --release -- --level 1-1 --autoplay --screenshot docs/level11.png --frames 300
```

Sin GPU se puede usar Vulkan por software (Mesa lavapipe) bajo Xvfb:
`xvfb-run -a cargo run -- --screen menu --screenshot docs/menu.png --frames 60`.
