import { mount } from 'svelte';
import App from './App.svelte';
import ViewerWindow from './lib/ViewerWindow.svelte';

// The pop-out viewer window loads the same bundle with ?viewer=1 and shows only the viewer.
const viewerOnly = new URLSearchParams(location.search).get('viewer') === '1';
if (viewerOnly) document.body.style.cssText = 'margin:0;background:#111'; // dark ground for the viewer window only
mount(viewerOnly ? ViewerWindow : App, { target: document.getElementById('app')! });
