%global crate posting-import

Name:           %{crate}
Version:        0.1.0
Release:        1%{?dist}
Summary:        Import API collections to Posting TUI format
License:        (MIT OR Apache-2.0)
URL:            https://github.com/yourusername/posting-import
Source0:        %{url}/archive/v%{version}/%{crate}-%{version}.tar.gz
BuildRequires:  rust-rpm-macros

%description
posting-import is a CLI tool that imports API collections from popular API clients
(Postman, Insomnia, Bruno) and converts them to the Posting TUI's YAML format.

Features:
* Import Postman collections (v2.0/v2.1)
* Import Insomnia collections (v4/v5)
* Import Bruno collections (.bru and OpenCollection YAML)
* Preserves folder structure and request metadata
* Supports authentication configurations

%package        bash-completion
Summary:        Bash completion for %{name}
Requires:       bash-completion
Provides:       %{name}-bash-completion = %{version}
BuildArch:      noarch

%package        fish-completion
Summary:        Fish completion for %{name}
Requires:       fish
Provides:       %{name}-fish-completion = %{version}
BuildArch:      noarch

%package        zsh-completion
Summary:        Zsh completion for %{name}
Requires:       zsh
Provides:       %{name}-zsh-completion = %{version}
BuildArch:      noarch

%prep
%autosetup -p1

%build
cargo build --release --locked

%install
# Install binary
install -Dm755 target/release/%{crate} %{buildroot}%{_bindir}/%{crate}

# Install shell completions
install -Dm644 completions/%{crate}.bash %{buildroot}%{_datadir}/bash-completion/completions/%{crate}
install -Dm644 completions/%{crate}.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/%{crate}.fish
install -Dm644 completions/_%{crate} %{buildroot}%{_datadir}/zsh/site-functions/_%{crate}

# Install man page
install -Dm644 man/%{crate}.1 %{buildroot}%{_mandir}/man1/%{crate}.1

# Install documentation
install -Dm644 README.md %{buildroot}%{_docdir}/%{crate}/README.md
install -Dm644 LICENSE %{buildroot}%{_docdir}/%{crate}/LICENSE

%check
cargo test --release --locked

%files
%{_bindir}/%{crate}
%{_mandir}/man1/%{crate}.1*
%doc %{_docdir}/%{crate}/README.md
%doc %{_docdir}/%{crate}/LICENSE
%license %{_docdir}/%{crate}/LICENSE

%files bash-completion
%{_datadir}/bash-completion/completions/%{crate}

%files fish-completion
%{_datadir}/fish/vendor_completions.d/%{crate}.fish

%files zsh-completion
%{_datadir}/zsh/site-functions/_%{crate}

%changelog
* Mon Jan 01 2024 Your Name <your.email@example.com> - 0.1.0-1
- Initial package release
