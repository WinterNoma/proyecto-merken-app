# Informe: Export JSON (interfaz)

Se añadieron el contrato TypeScript, la acción de exportación, sus estados, la ruta seleccionable, la copia opcional y pruebas de interfaz.

La comprobación de tipos mediante la API de TypeScript en el proceso terminó sin diagnósticos. Windows denegó la creación de procesos (Access is denied / EPERM), por lo que no fue posible ejecutar `npx tsc --noEmit`, `npx vite build`, `npm run test:ui` ni `git status`. La importación de las pruebas en el proceso alternativo tampoco pudo ejecutarse porque rolldown requiere `process`, que no está disponible en ese entorno. Las pruebas y el build quedan pendientes de ejecución real.
