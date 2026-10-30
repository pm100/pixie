<script lang="ts">
  import { onMount } from 'svelte';
  import { suggestHome } from './api';
  import { formatGps } from './format';
  import HomePicker from './HomePicker.svelte';
  import { cleanTripPrefs, fromKm, MAX_NIGHTS, MAX_RADIUS_KM, MAX_STOP_KM, toKm, unitFor, type TripPrefs } from './trips';

  let { prefs, onprefs }: { prefs: TripPrefs; onprefs: (p: TripPrefs) => void } = $props();

  // The home being looked at: the saved one, or Pixie's guess, or what was last clicked. Saved only when confirmed.
  // svelte-ignore state_referenced_locally -- only the starting point; the draft then changes on its own
  let draft = $state<{ lat: number; lon: number } | null>(prefs.home);
  let guessed = $state(false), guessFailed = $state(false), error = $state('');
  const changed = $derived(draft !== null && (!prefs.confirmed || prefs.home?.lat !== draft.lat || prefs.home?.lon !== draft.lon));

  onMount(async () => {
    if (draft) return;
    try {
      const g = await suggestHome();
      if (g) { draft = { lat: g[0], lon: g[1] }; guessed = true; } else guessFailed = true;
    } catch (e) { error = String(e); }
  });

  const pick = (lat: number, lon: number) => { draft = { lat, lon }; guessed = false; };
  const confirm = () => { if (draft) onprefs(cleanTripPrefs({ ...prefs, home: draft, confirmed: true })); };
  // Distances are kept in kilometres and shown in miles where people use them (the UK, the US, ...).
  const unit = unitFor(typeof navigator === 'undefined' ? undefined : navigator.language);
  const unitName = unit === 'mi' ? 'miles' : 'km';
  const setRadius = (v: string) => onprefs(cleanTripPrefs({ ...prefs, radiusKm: toKm(Number(v), unit) }));
  const setStop = (v: string) => onprefs(cleanTripPrefs({ ...prefs, stopKm: toKm(Number(v), unit) }));
  const setNights = (v: string) => onprefs(cleanTripPrefs({ ...prefs, minNights: Number(v) }));
</script>

<h2>Trips</h2>
<p>A trip is a run of days spent away from home. A day at home ends a trip. Photos and videos without a position are included when their date falls inside a trip.</p>

<h3>Where is home?</h3>
{#if guessed}<p>Pixie guesses home is where most of your photos were taken, shown on the map. Click the map to move it, then confirm.</p>
{:else if guessFailed}<p>Pixie could not guess, because none of your photos has a position yet. Click the map to set home.</p>
{:else}<p>Click the map to move home.</p>{/if}
<HomePicker home={draft} radiusKm={prefs.radiusKm} onpick={pick} />
{#if draft}<p class="where">{formatGps(draft.lat, draft.lon)}</p>{/if}
{#if changed}<button class="primary" onclick={confirm} title="Use the place on the map as home">{prefs.confirmed ? 'Use this as home' : 'This is home'}</button>
{:else if prefs.confirmed}<p class="ok">Home is set.</p>{/if}
{#if error}<p class="err">{error}</p>{/if}

<h3>What counts as a trip</h3>
<label class="row">Distance from home that counts as away ({unitName}):
  <input class="num" type="number" min="1" max={Math.round(fromKm(MAX_RADIUS_KM, unit))} step="any" value={fromKm(prefs.radiusKm, unit)}
         onchange={(e) => setRadius(e.currentTarget.value)} />
</label>
<label class="row">Fewest nights away that make a trip:
  <input class="num" type="number" min="0" max={MAX_NIGHTS} step="1" value={prefs.minNights}
         onchange={(e) => setNights(e.currentTarget.value)} />
</label>
<p class="note">0 counts a day out as a trip.</p>

<h3>Where a trip went</h3>
<p>Stops name the trip's destination and are the numbered points on the map. Photos taken close together count as one stop.</p>
<label class="row">Distance within which photos are one stop ({unitName}):
  <input class="num" type="number" min="1" max={Math.round(fromKm(MAX_STOP_KM, unit))} step="any" value={fromKm(prefs.stopKm, unit)}
         onchange={(e) => setStop(e.currentTarget.value)} />
</label>

<style>
  .row { display: flex; align-items: center; gap: 8px; margin: 0 0 8px; }
  .note { color: #666; font-size: 12px; margin: 0 0 8px; }
  h3 { margin: 16px 0 6px; font-size: 14px; }
  .num { width: 5em; font: inherit; padding: 2px 4px; }
  .where { color: #555; font-size: 12px; margin: 0 0 8px; }
  .ok { color: #2a7a2a; }
  .err { color: #b00020; }
</style>
