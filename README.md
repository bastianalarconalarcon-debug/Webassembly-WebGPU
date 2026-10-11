# Mi Primer Computador

Juego para niñas y niños desde los 3 años que todavía no saben leer. Sirve
para explorar el teclado y el mouse sin miedo: cada tecla y cada clic hace
algo, nada se rompe y no hay forma de perder. Todo se explica con voz en
español de Chile, dibujos y sonidos.

Está hecho con **Rust compilado a WebAssembly** (toda la lógica) y
**WebGPU** (todos los gráficos).

## Los juegos

| Juego | Qué aprende |
| --- | --- |
| ⌨️ **Teclado mágico** | Cada letra aparece gigante con un dibujo ("¡Be! ¡Ballena!"). Los números cuentan objetos ("uno, dos, tres… ¡tres manzanas!"). Espacio, Enter, Borrar y las flechas tienen nombre y efecto propio. |
| ✏️ **Escribir** | Escribe palabras con dibujo (su nombre primero, después SOL, OSO, MAMÁ, PAPÁ…). Un teclado en pantalla ilumina la tecla que hay que buscar. Si se equivoca dos veces, cualquier tecla ayuda. |
| ➕ **Sumar** | Primero contar ("¿Cuántas mariposas hay?"), después sumas con objetos hasta 5 y hasta 10. Los números traen puntitos como un dado. Si se equivoca, el juego cuenta con ella. |
| 🔺 **Formas y colores** | "¿Dónde está la estrella roja?" Si toca otra, la voz dice qué tocó. Primero cambia sólo el color, después la forma y después ambos. |
| 🎨 **Pintar** | Pinta con sólo mover el mouse. Clic = estampa. Barra de colores, arcoíris, goma y basurero. La rueda del mouse cambia el tamaño del pincel. |
| 🎵 **Música** | Xilófono de 8 barras. Pasar el mouse = glissando. Cinco instrumentos. "Estrellita" y "Los pollitos dicen": cada tecla toca la siguiente nota de la canción. |
| 🫧 **Burbujas** | Burbujas con animales: al reventarlas dicen su nombre y su sonido ("¡Vaca! ¡Muuu!"). Apretar una letra revienta el animal que empieza con esa letra. |
| 🧱 **Construir** | Mundo 3D de bloques tipo Minecraft sobre una pradera (zona de 24×24). Clic para poner bloques (o arrastrar para hacer filas), goma o clic derecho para sacar, ↩️ deshacer. Bloques de colores, de naturaleza (pasto, madera, piedra, arena, agua…) y sorpresas (vidrio, oro, nube, árbol, casita). Interruptor 🎈 Flotar / 🍎 Caer: con física, lo que queda sin apoyo tiembla y se cae (un puente puede sobresalir hasta 3 bloques). Los números arman una torre contando. |
| 🐝 **La abeja** | Se mueve con las flechas (o WASD) y junta flores contando hasta 10. Unas flechas en pantalla se iluminan igual que las del teclado. |

Detalles pensados para los 3 años:

- Cursor grande y de color, con un brillo que lo sigue.
- La voz nunca dice "mal" ni "error". Si se equivoca, le dice qué tocó y la ayuda.
- Si aprieta muchas teclas a la vez, la voz no se atropella: dice lo último.
- Si no toca nada por un rato, la voz la invita a seguir.

## Cómo jugar (para papá o mamá)

1. Abre el juego en **Microsoft Edge** o **Google Chrome** en Windows.
   Edge trae voces neuronales de Chile (Catalina y Lorenzo) que suenan muy naturales.
2. Escribe el nombre de tu hija o hijo (opcional): aparece en la pantalla de inicio y es la primera palabra en *Escribir*.
3. Elige la voz con **Probar** y aprieta **¡A jugar!**.
4. Con "Pantalla completa" activado, el juego captura teclas como Esc, Ctrl+W o la tecla Windows para que no se salga sin querer.
5. **Para salir:** mantén presionada la tecla **Esc** unos segundos.

## Cómo ejecutarlo en tu computador

El juego ya viene compilado en `web/`. Sólo hace falta servir esa carpeta
con cualquier servidor web (abrir el `index.html` con doble clic no funciona,
porque el navegador no deja cargar WebAssembly desde archivos locales):

```sh
cd web
python -m http.server 8000      # en Windows también: py -m http.server 8000
```

Luego abre <http://localhost:8000> en Edge o Chrome.

## Para programar

Requisitos: [Rust](https://rustup.rs) con el target de WebAssembly.

```sh
rustup target add wasm32-unknown-unknown
./build.sh        # compila y copia el resultado a web/juego.wasm
cargo test        # pruebas (incluye una "niña virtual" que aprieta todo al azar)
```

### Cómo está hecho

```
src/                 Rust -> WebAssembly: toda la lógica
  lib.rs             funciones que llama JavaScript (sin wasm-bindgen)
  game.rs            estado compartido, entrada, botón de casa, transiciones
  gfx.rs             lista de figuras que se dibujan cada cuadro
  audio.rs           cola de sonidos y frases para la voz
  fx.rs              partículas (confeti, chispas, fuegos artificiales)
  vocab.rs           vocabulario chileno: letras, palabras, animales, colores
  gfx3d.rs           3D: cubos instanciados, cámara, matrices, oclusión ambiental
  voxel.rs           mundo de bloques: rayo, física "andamio", deshacer
  modes/             un archivo por juego
web/                 el navegador
  index.html         portada para papá/mamá
  main.js            carga el wasm, conecta mouse/teclado, bucle principal
  renderer.js        WebGPU: una sola llamada de dibujo por cuadro (instancing)
  shaders.wgsl       figuras por funciones de distancia, fondos animados
  voxels.js          WebGPU 3D: sombras, antialiasing, calidad automática
  cubos.wgsl         bloques con texturas pixeladas procedurales, cielo, pradera
  atlas.js           emojis y letras en una textura (letras como SDF)
  audio.js           sintetizador con Web Audio (no hay archivos de sonido)
  speech.js          voz en español (SpeechSynthesis)
```

Rust no toca el navegador: cada cuadro llena un arreglo de figuras
(posición, tamaño, color, tipo) y una cola de eventos de sonido/voz.
JavaScript copia ese arreglo a un buffer de WebGPU y lo dibuja de una vez;
el fragment shader calcula cada figura (círculo, estrella, corazón, letra…)
con funciones de distancia, así todo se ve nítido a cualquier tamaño.

Para agregar palabras, animales o colores basta con editar `src/vocab.rs`.
Para un juego nuevo: un archivo en `src/modes/` que implemente `Activity`,
y una puerta en `src/modes/home.rs`.

## Ideas para después

- Trazar letras con el mouse.
- Más canciones y un "Simón dice" musical.
