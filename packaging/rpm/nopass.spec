# RPM package for Fedora, RHEL and anything else that speaks rpm.
#
# Quickest route to users is COPR, which builds this from a source RPM and
# hosts the repository for you — see ../README.md. Fedora proper wants the
# dependency-bundling review, which this spec declares honestly below.
Name:           nopass
Version:        0.2.0
Release:        1%{?dist}
Summary:        Fast, self-contained password manager

License:        MIT
URL:            https://github.com/souravsspace/nopass
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
Recommends:     git
Suggests:       xclip
Suggests:       wl-clipboard

# Rust crates are linked statically, so the binary carries its dependencies.
Provides:       bundled(crate(age))

%global _description %{expand:
nopass keeps each password in its own age-encrypted file under one
directory. Every command that reads or changes the store asks for your
master passphrase; reads can reuse a cached one when you opt in.}

%description %{_description}

%prep
%autosetup -n %{name}-%{version}
cargo fetch --locked

%build
cargo build --release --frozen --package nopass-cli

%check
# Hermetic: no network, no real store, temp dirs throughout.
cargo test --frozen --workspace

%install
install -Dpm 0755 target/release/nopass %{buildroot}%{_bindir}/nopass

%files
%license LICENSE
%doc README.md CONTRIBUTING.md
%{_bindir}/nopass

%changelog
* Fri Aug 07 2026 Sourav <souravsspace@gmail.com> - 0.2.0-1
- Every command that changes the store now authenticates
- Opt-in in-memory passphrase cache for reads
- Security fixes: absolute-path escape, edit plaintext permissions, agent hardening

* Tue Jun 10 2026 Sourav <souravsspace@gmail.com> - 0.1.0-1
- First release
