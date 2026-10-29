<script lang="ts">
  /** Settings, Ask Pixie: the switch, and in plain words what is and is not sent. */
  let { on, onchange, library, onlibrary }: {
    on: boolean; onchange: (on: boolean) => void;
    /** Whether Ask Pixie may also be told the names in the library (tags, named people, trips), and a change to that. */
    library: boolean; onlibrary: (on: boolean) => void;
  } = $props();
</script>

<h2>Ask Pixie</h2>
<label class="opt"><input type="checkbox" checked={on} onchange={(e) => onchange(e.currentTarget.checked)} /> Let me ask Pixie questions and have it change settings</label>
<p class="strong">Your pictures never leave this computer.</p>
<p>When you ask a question, it is sent over the internet to a server, where a language model writes the answer. Pixie does not send anything until you press Ask, and nothing is changed until you press Apply.</p>
<h3>Share the names in my library</h3>
<label class="opt"><input type="checkbox" checked={library} disabled={!on} onchange={(e) => onlibrary(e.currentTarget.checked)} /> Let Ask Pixie see the names of my tags, the people I have named, and my trips</label>
<p>Ask Pixie answers many questions better when it can see these. It can tell that “Rose” is a person in your library and not a flower, find “the Italy trip”, and say when a trip was. Without them it has to guess, and Pixie checks the names on this computer after it answers.</p>
<p>This is off until you turn it on, and it does nothing while Ask Pixie itself is off. The lists below show exactly what it adds.</p>
<h3>Always sent</h3>
<ul>
  <li>The words you type, and the last few messages of the conversation.</li>
  <li>Pixie's settings and their current values, and which tab you were on.</li>
  <li>Today's date, your language and your system (Windows or Mac).</li>
  <li>A random number, made on this computer, that counts how many questions are asked in a day. It is not linked to you.</li>
</ul>
<h3>Sent only if you turn on sharing</h3>
<ul>
  <li>The names of your tags, with how many photos each has.</li>
  <li>The names of the people you have named, with how many photos each is in.</li>
  <li>Your trips: their dates, the name you gave them, the town they went to, how many nights, and how many photos and videos.</li>
  <li>How many photos and videos are in your library, and how many are picked.</li>
</ul>
<h3>Never sent, whatever you choose</h3>
<ul>
  <li>Pictures or videos, thumbnails, or anything made from them, such as faces.</li>
  <li>File names, folder names, or where your files are.</li>
  <li>Where home is, or any map coordinates.</li>
  <li>Your email address or contacts.</li>
</ul>
<p class="hint">Turning Ask Pixie off stops all questions. It is also off after "Start again from a clean slate".</p>

<style>
  .strong { font-weight: 600; }
  h3 { margin: 16px 0 4px; font-size: 14px; }
  ul { margin: 0 0 8px; padding-left: 20px; }
  li { margin: 2px 0; }
  .opt { display: block; margin: 6px 0 12px; }
  .hint { color: #666; font-size: 13px; }
</style>
