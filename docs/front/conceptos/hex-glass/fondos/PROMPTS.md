# Hibrido elegido: exploracion de fondos

El usuario eligio el concepto hibrido como base visual provisional y rechazo el fondo rocoso. Quiere explorar texturas abstractas en movimiento que respondan a la interfaz. Esto documenta una direccion de diseno, no una implementacion ni una eleccion definitiva de fondo.

Generado con image_gen integrado. Los archivos son imagenes estaticas; no contienen animacion ni interaccion.

- 01-flujo-satinado.png: corrientes abstractas amplias, suaves y oscuras.
- 02-particulas-en-flujo.png: particulas finas y filamentos en los bordes.
- 03-ondas-difusas.png: ondas de material translúcido con bordes desenfocados.
- 04-hibrido-sin-rocas.png: revision del concepto elegido con el paisaje retirado; conserva la interfaz.

Propuesta por evaluar: desplazamiento casi imperceptible en reposo, onda local al seleccionar una zona, reaccion junto al microfono durante la captura real de audio. Las reacciones decorativas se ligan a acciones del usuario y no simulan mediciones. Mantener los estados de salud en tarjetas y zonas; el fondo permanece neutral. Respetar movimiento reducido y detener la animacion fuera de vista. Rendimiento, contraste y comodidad pendientes de verificar cuando exista un prototipo.

## Prompts completos

### 01-flujo-satinado

```text
Use case: stylized-concept. Asset: background-only visual concept for PULSYS, a calm mobile health-monitoring application. Produce ONE edge-to-edge portrait 9:16 abstract background image. This is a still frame suggesting slow, continuous motion to implement later in code, NOT an animation file. No interface, no lettering, no words, no icons, no panels, no body, no logos, no labels, no borders, no device frame. Palette strictly near-black and charcoal with restrained silver-gray light; no bright or saturated colors. Very dark quiet upper third and central zone because white text and a centered body diagram will later overlay them; keep most detail near the peripheral margins and lower third. Subtle depth, smooth gradients, sophisticated restraint, excellent image quality. The texture itself may be low contrast so the eventual app text remains dominant. Soft ambient lighting. Avoid rocks, mountains, landscapes, space scenes, stars, planets, cloudy nebulae, photographic scenery, harsh shadows, shiny chrome, neon, HUD grids, repeated fake ECG/heartbeat signals, chart curves, medical claims, clutter, noise, watermarks.
Direction 1: continuous fluid field. Broad, softly folded satin-like charcoal light currents, abstract and weightless, a few smooth dark translucent ribbons bending gently along left and bottom edges, barely perceptible diffusion and fine tactile texture. Atmospheric but not foggy. Suggest a very slow breathing-like expansion, almost still. Center stays smooth near-black. The sophistication comes from spaciousness and extremely restrained rim illumination. Do not depict literal fabric, solid objects, or smoke plumes.
```

### 02-particulas-en-flujo

```text
Use case: stylized-concept. Asset: background-only visual concept for PULSYS, a calm mobile health-monitoring application. Produce ONE edge-to-edge portrait 9:16 abstract background image. This is a still frame suggesting slow, continuous motion to implement later in code, NOT an animation file. No interface, no lettering, no words, no icons, no panels, no body, no logos, no labels, no borders, no device frame. Palette strictly near-black and charcoal with restrained silver-gray light; no bright or saturated colors. Very dark quiet upper third and central zone because white text and a centered body diagram will later overlay them; keep most detail near the peripheral margins and lower third. Subtle depth, smooth gradients, sophisticated restraint, excellent image quality. The texture itself may be low contrast so the eventual app text remains dominant. Soft ambient lighting. Avoid rocks, mountains, landscapes, space scenes, stars, planets, cloudy nebulae, photographic scenery, harsh shadows, shiny chrome, neon, HUD grids, repeated fake ECG/heartbeat signals, chart curves, medical claims, clutter, noise, watermarks.
Direction 2: a quiet computational particle field, suited to monitoring and connected readings. Sparse microscopic soft gray particles, with only a few hair-thin curved filaments forming broad drifting flows at far left and lower right. No fixed lattice, no circuit board, no starfield. Particles should feel like a fine suspended material close to the viewer, not outer space. Around one lower peripheral point the flow gently curves as if responding to a touch. Huge expanses of calm charcoal negative space, no strong focal dot, no bright specks.
```

### 03-ondas-difusas

```text
Use case: stylized-concept. Asset: background-only visual concept for PULSYS, a calm mobile health-monitoring application. Produce ONE edge-to-edge portrait 9:16 abstract background image. This is a still frame suggesting slow, continuous motion to implement later in code, NOT an animation file. No interface, no lettering, no words, no icons, no panels, no body, no logos, no labels, no borders, no device frame. Palette strictly near-black and charcoal with restrained silver-gray light; no bright or saturated colors. Very dark quiet upper third and central zone because white text and a centered body diagram will later overlay them; keep most detail near the peripheral margins and lower third. Subtle depth, smooth gradients, sophisticated restraint, excellent image quality. The texture itself may be low contrast so the eventual app text remains dominant. Soft ambient lighting. Avoid rocks, mountains, landscapes, space scenes, stars, planets, cloudy nebulae, photographic scenery, harsh shadows, shiny chrome, neon, HUD grids, repeated fake ECG/heartbeat signals, chart curves, medical claims, clutter, noise, watermarks.
Direction 3: soft diffused interaction ripples in smoked glass. Two or three very broad, blurred nested contour waves expanding from just outside the bottom-right edge, almost disappearing into near-black. Gentle optical interference between transparent charcoal layers, restrained silvery gradients at outer margins, no hard circular targets and no technical graphics. Suggest that selecting a card sends one slow dissipating ripple through the material. The upper and center remain quiet and dark. No shiny chrome and no landscape.
```

### 04-hibrido-sin-rocas

```text
Use case: precise-object-edit / ui-mockup. Edit the provided PULSYS hybrid mobile app screenshot. Preserve its exact UI layout, screen proportions, typography, all English text, all demo numbers and units, all buttons, glass cards, body silhouette, and margins. Change ONLY the background treatment: remove every rock, terrain and mountain at the bottom completely. Replace the background with a smooth near-black charcoal abstract field with extremely subtle flowing satin-like light gradients along the outside margins. Give a slight sense of layered depth and material in motion, but it must read as calm, nearly still and spacious behind the sharp interface. Keep the central and upper background very dark. A very faint dissipating soft ripple may sit immediately behind the microphone capsule as an example of a user-triggered interaction, without overlapping text or adding controls. Refine away grain/noise. No landscape, stars, space imagery, planets, neon, medical waveforms or bright colors. Preserve high white-text contrast, and do not make the body glow brighter. One complete flat front-view mobile screen, no phone frame, no captions outside UI. This is an image concept only.
```
