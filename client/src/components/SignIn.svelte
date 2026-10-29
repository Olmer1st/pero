<script lang="ts">
  import { session } from "../lib/session.svelte.ts";

  let token = $state("");
  let error = $state("");
  let busy = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (token.trim() === "") {
      error = "Paste the device token first.";
      return;
    }
    busy = true;
    error = (await session.signIn(token)) ?? "";
    busy = false;
    if (error === "") token = "";
  }
</script>

<section class="card narrow">
  <h1>Sign in</h1>
  <p class="muted">
    This device needs a token from the server owner. It is kept in memory only: reloading the page
    signs you out.
  </p>
  {#if session.notice}
    <p class="notice" role="alert">{session.notice}</p>
  {/if}
  <form onsubmit={submit}>
    <label for="token">Device token</label>
    <input
      id="token"
      name="pero-device-token"
      type="password"
      autocomplete="current-password"
      spellcheck="false"
      bind:value={token}
      placeholder="pero_…"
    />
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <button type="submit" disabled={busy}>{busy ? "Checking…" : "Sign in"}</button>
  </form>
</section>
