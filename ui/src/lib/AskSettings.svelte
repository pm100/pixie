<script lang="ts">
  /** Settings, Ask Pixie: the switch, and in plain words what is and is not sent. */
  let { on, onchange }: { on: boolean; onchange: (on: boolean) => void } = $props();
</script>

<h2>Ask Pixie</h2>
<label class="opt"><input type="checkbox" checked={on} onchange={(e) => onchange(e.currentTarget.checked)} /> Let me ask Pixie questions and have it change settings</label>
<p class="strong">Your pictures never leave this computer.</p>
<p>When you ask a question, it is sent over the internet to a server, where a language model writes the answer. Pixie does not send anything until you press Ask, and nothing is changed until you press Apply.</p>
<h3>Always sent</h3>
<ul>
  <li>The words you type, and the last few messages of the conversation.</li>
  <li>Pixie's settings and their current values, and which tab you were on.</li>
  <li>Today's date, your language and your system (Windows or Mac).</li>
  <li>A random number, made on this computer, that counts how many questions are asked in a day. It is not linked to you.</li>
</ul>
<h3>Never sent</h3>
<ul>
  <li>Pictures or videos, thumbnails, or anything made from them, such as faces.</li>
  <li>File names, folder names, or where your files are.</li>
  <li>The names of your tags, the people you have named, or your trips.</li>
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
