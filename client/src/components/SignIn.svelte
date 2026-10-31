<script lang="ts">
  import { isDesktop } from "../lib/api.ts";
  import { session } from "../lib/session.svelte.ts";

  let token = $state("");
  let error = $state("");
  let busy = $state(false);
  let address = $state("");

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

  async function tryAddress(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    await session.tryAddress(address);
  }
</script>

<section class="card narrow">
  <h1>Sign in</h1>

  {#if isDesktop()}
    {#if session.server?.status === "searching"}
      <p class="muted" role="status">Looking for the Pero server…</p>
    {:else if session.server?.status === "found"}
      <p class="muted" role="status">Connected to {session.server.address} (server {session.server.version}).</p>
    {:else if session.server?.status === "missing"}
      <p class="error" role="alert">{session.server.error}</p>
      <form class="inline" onsubmit={tryAddress}>
        <input type="text" bind:value={address} placeholder="writer.local:8443" aria-label="Server address" spellcheck="false" />
        <button type="submit">Try this address</button>
        <button type="button" class="secondary" onclick={() => session.tryAddress("writer.local:8443")}>Search again</button>
      </form>
    {/if}
  {/if}

  {#if !isDesktop() || session.server?.status === "found"}
    <p class="muted">
      This device needs a token from the server owner.
      {#if isDesktop()}
        It is kept in this computer's keychain, so you only sign in once.
      {:else}
        It is kept in memory only: reloading the page signs you out.
      {/if}
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
  {/if}
</section>
