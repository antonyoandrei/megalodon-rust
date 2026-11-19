// Browser bridge only; interaction logic is authored in client/src/lib.rs.
import init from './megalodon_ui.js';
init().catch(error => console.warn('Megalodon: mejoras de interacción no disponibles.', error));
