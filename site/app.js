// "Copy" buttons on the command blocks.
document.querySelectorAll('button.copy').forEach((button) => {
  button.addEventListener('click', async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = 'Copied!';
    } catch {
      button.textContent = 'Select and copy';
    }
    setTimeout(() => { button.textContent = 'Copy'; }, 1800);
  });
});

// Shows the latest release's version. Until a release is published, the main
// button points to building from source instead.
fetch('https://api.github.com/repos/victorlcampos/rabisco/releases/latest', {
  headers: { Accept: 'application/vnd.github+json' },
})
  .then((response) => {
    if (response.status === 404) {
      const button = document.getElementById('download');
      button.href = '#install';
      button.querySelector('.label').textContent = 'Install on macOS';
      document.getElementById('card-download').classList.add('hidden');
      document.querySelector('.install-grid').classList.add('single');
      return null;
    }
    return response.ok ? response.json() : null;
  })
  .then((release) => {
    if (!release || !release.tag_name) return;
    const note = document.getElementById('download-note');
    note.textContent = `Version ${release.tag_name.replace(/^v/, '')} · ${note.textContent}`;
  })
  .catch(() => {});
