// Botões "Copiar" dos blocos de comando.
document.querySelectorAll('button.copy').forEach((button) => {
  button.addEventListener('click', async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = 'Copiado!';
    } catch {
      button.textContent = 'Selecione e copie';
    }
    setTimeout(() => { button.textContent = 'Copiar'; }, 1800);
  });
});

// Mostra a versão da última release. Enquanto não houver nenhuma publicada,
// o botão principal leva para a instalação a partir do código.
fetch('https://api.github.com/repos/victorlcampos/rabisco/releases/latest', {
  headers: { Accept: 'application/vnd.github+json' },
})
  .then((response) => {
    if (response.status === 404) {
      const button = document.getElementById('download');
      button.href = '#instalar';
      button.querySelector('.label').textContent = 'Instalar no macOS';
      document.getElementById('card-download').classList.add('hidden');
      document.querySelector('.install-grid').classList.add('single');
      return null;
    }
    return response.ok ? response.json() : null;
  })
  .then((release) => {
    if (!release || !release.tag_name) return;
    const note = document.getElementById('download-note');
    note.textContent = `Versão ${release.tag_name.replace(/^v/, '')} · ${note.textContent}`;
  })
  .catch(() => {});
