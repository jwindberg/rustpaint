#!/usr/bin/env python3
"""Convert HaikuPaint's translations (Haiku catkeys) to RustPaint's Fluent
files.

Each RustPaint message is mapped to the HaikuPaint string it comes from; its
translations come from that string's catkeys entries, with HaikuPaint's name
changed to RustPaint's. English uses RustPaint's own wording (MacPaint's
title case). The few messages HaikuPaint has no string for carry their own
translations below.

Writes locales/<language>.ftl, relative to the RustPaint project root.

Usage: python3 -I tools/catkeys_to_ftl.py <HaikuPaint/locales> [project-root]
"""
import os
import sys

# Catkeys language names to Fluent locale codes.
LANGUAGES = {
    'german': 'de',
    'french': 'fr',
    'spanish': 'es',
    'swedish': 'sv',
}

# id, RustPaint's English, HaikuPaint's English (None: no HaikuPaint string).
# Placeables: HaikuPaint's %s and %d become the variable named in the
# English text.
MESSAGES = [
    # Menu titles.
    ('menu-file', 'File', 'File'),
    ('menu-edit', 'Edit', 'Edit'),
    ('menu-goodies', 'Goodies', 'Goodies'),
    ('menu-font', 'Font', 'Font'),
    ('menu-font-size', 'FontSize', 'FontSize'),
    ('menu-style', 'Style', 'Style'),
    # File.
    ('about', 'About RustPaint…', 'About HaikuPaint…'),
    ('new', 'New', 'New'),
    ('open', 'Open…', 'Open…'),
    ('save', 'Save', 'Save'),
    ('save-as', 'Save As…', 'Save as…'),
    ('revert', 'Revert', 'Revert'),
    ('export-png', 'Export as PNG…', 'Export as PNG…'),
    ('print', 'Print…', 'Print…'),
    ('print-catalog', 'Print Catalog…', 'Print catalog…'),
    ('quit', 'Quit', 'Quit'),
    # Edit.
    ('undo', 'Undo', 'Undo'),
    ('cut', 'Cut', 'Cut'),
    ('copy', 'Copy', 'Copy'),
    ('paste', 'Paste', 'Paste'),
    ('clear', 'Clear', 'Clear'),
    ('select-all', 'Select All', 'Select all'),
    ('invert', 'Invert', 'Invert'),
    ('fill', 'Fill', 'Fill'),
    ('trace-edges', 'Trace Edges', 'Trace edges'),
    ('flip-horizontal', 'Flip Horizontal', 'Flip horizontal'),
    ('flip-vertical', 'Flip Vertical', 'Flip vertical'),
    ('rotate', 'Rotate', 'Rotate'),
    # Goodies.
    ('grid', 'Grid', 'Grid'),
    ('fat-bits', 'FatBits', 'FatBits'),
    ('show-page', 'Show Page', 'Show page'),
    ('edit-pattern', 'Edit Pattern…', 'Edit pattern…'),
    ('brush-shape', 'Brush Shape…', 'Brush shape…'),
    ('brush-mirrors', 'Brush Mirrors…', 'Brush mirrors…'),
    # FontSize and Style.
    ('font-size-points', '{ $size } point', '%d point'),
    ('plain', 'Plain', 'Plain'),
    ('bold', 'Bold', 'Bold'),
    ('italic', 'Italic', 'Italic'),
    ('underline', 'Underline', 'Underline'),
    ('outline', 'Outline', 'Outline'),
    ('shadow', 'Shadow', 'Shadow'),
    ('align-left', 'Align Left', 'Align left'),
    ('align-middle', 'Align Middle', 'Align middle'),
    ('align-right', 'Align Right', 'Align right'),
    # Dialogs.
    ('ok', 'OK', 'OK'),
    ('cancel', 'Cancel', 'Cancel'),
    ('none', 'None', 'None'),
    ('about-version', 'RustPaint version { $version }',
     'HaikuPaint version 2.0'),
    ('about-credit', 'after MacPaint, written by Bill Atkinson',
     'after MacPaint, written by Bill Atkinson'),
    ('about-copyright', 'MacPaint copyright 1983, Apple Computer Inc.',
     'MacPaint copyright 1983, Apple Computer Inc.'),
    # Alerts.
    ('error', 'Error', 'Error'),
    ('unsaved-changes', 'Unsaved changes', 'Unsaved changes'),
    ('save-changes', 'Save changes to the document "{ $name }"?',
     'Save changes to the document "%s"?'),
    ('dont-save', "Don't Save", "Don't save"),
    ('revert-question', 'Revert to the last version saved?',
     'Revert to the last version saved?'),
    ('untitled', 'untitled', 'untitled'),
    ('no-documents', 'There are no MacPaint documents in this folder.',
     'There are no MacPaint documents in this folder.'),
    ('catalog-heading', 'MacPaint documents in { $folder }',
     'MacPaint documents in %s'),
    ('print-catalog-title', 'Print Catalog', 'HaikuPaint: Print catalog'),
    # Documents.
    ('error-open', "Couldn't open the file: { $error }",
     "Couldn't open the file: %s"),
    ('error-save', "Couldn't save the document: { $error }",
     "Couldn't save the document: %s"),
    ('error-export', "Couldn't export the image: { $error }",
     "Couldn't export the image: %s"),
    ('error-unreadable', "RustPaint can't read this kind of file.",
     "HaikuPaint can't read this kind of file."),
    ('error-too-short', 'The file is too short to be a MacPaint document.',
     'The file is too short to be a MacPaint document.'),
    ('error-not-macpaint', "This doesn't look like a MacPaint document.",
     "This doesn't look like a MacPaint document."),
    ('error-damaged', 'The MacPaint document is damaged or incomplete.',
     'The MacPaint document is damaged or incomplete.'),
    ('filter-macpaint-document', 'MacPaint document', 'MacPaint document'),
    # RustPaint's own.
    ('error-print', "Couldn't print: { $error }", None),
    ('error-print-open', "Couldn't open the PDF to print it: { $error }",
     None),
    ('filter-macpaint-documents', 'MacPaint documents', None),
    ('filter-images', 'Images', None),
    ('filter-all', 'All files', None),
    ('filter-png', 'PNG image', None),
]

# Translations of the messages HaikuPaint has no string for.
OWN = {
    'de': {
        'error-print': 'Drucken nicht möglich: { $error }',
        'error-print-open':
            'Das PDF zum Drucken konnte nicht geöffnet werden: { $error }',
        'filter-macpaint-documents': 'MacPaint-Dokumente',
        'filter-images': 'Bilder',
        'filter-all': 'Alle Dateien',
        'filter-png': 'PNG-Bild',
    },
    'fr': {
        'error-print': "Impossible d'imprimer : { $error }",
        'error-print-open':
            "Impossible d'ouvrir le PDF pour l'imprimer : { $error }",
        'filter-macpaint-documents': 'Documents MacPaint',
        'filter-images': 'Images',
        'filter-all': 'Tous les fichiers',
        'filter-png': 'Image PNG',
    },
    'es': {
        'error-print': 'No se pudo imprimir: { $error }',
        'error-print-open':
            'No se pudo abrir el PDF para imprimirlo: { $error }',
        'filter-macpaint-documents': 'Documentos de MacPaint',
        'filter-images': 'Imágenes',
        'filter-all': 'Todos los archivos',
        'filter-png': 'Imagen PNG',
    },
    'sv': {
        'error-print': 'Kunde inte skriva ut: { $error }',
        'error-print-open':
            'Kunde inte öppna PDF-filen för utskrift: { $error }',
        'filter-macpaint-documents': 'MacPaint-dokument',
        'filter-images': 'Bilder',
        'filter-all': 'Alla filer',
        'filter-png': 'PNG-bild',
    },
}

HEADER = """\
# Generated by tools/catkeys_to_ftl.py from HaikuPaint's catkeys.
# Do not edit; change the script and re-run it instead.

"""


def read_catkeys(path):
    """The language's name and its translations, by English source."""
    with open(path, encoding='utf-8') as f:
        lines = f.read().splitlines()
    language = lines[0].split('\t')[1]
    translations = {}
    for line in lines[1:]:
        fields = line.split('\t')
        if len(fields) < 4:
            continue
        source = fields[0].replace('\\"', '"')
        translations.setdefault(source, fields[3].replace('\\"', '"'))
    return language, translations


def placeable(english):
    """The variable in an English message, as Fluent writes it."""
    start = english.find('{ $')
    return english[start:english.index('}', start) + 1] if start >= 0 else None


def convert(translation, english, source):
    """HaikuPaint's translation, made RustPaint's."""
    text = translation.replace('HaikuPaint', 'RustPaint')
    if source.startswith('HaikuPaint: '):
        # A window title; the name goes, as it does in English.
        text = text.split(': ', 1)[-1]
        text = text[:1].upper() + text[1:]
    variable = placeable(english)
    if variable is not None:
        text = text.replace('%s', variable).replace('%d', variable)
        if source == 'HaikuPaint version 2.0':
            text = text.replace('2.0', variable)
    return text


def write(path, messages):
    with open(path, 'w', encoding='utf-8') as f:
        f.write(HEADER)
        for key, text in messages:
            f.write(f'{key} = {text}\n')


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    source_dir = sys.argv[1]
    root = sys.argv[2] if len(sys.argv) > 2 else \
        os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out = os.path.join(root, 'locales')
    os.makedirs(out, exist_ok=True)

    write(os.path.join(out, 'en.ftl'),
          [(key, english) for key, english, _ in MESSAGES])

    for name in sorted(os.listdir(source_dir)):
        if not name.endswith('.catkeys') or name == 'en.catkeys':
            continue
        language, translations = read_catkeys(os.path.join(source_dir, name))
        code = LANGUAGES[language]
        messages = []
        for key, english, source in MESSAGES:
            if source is None:
                text = OWN[code][key]
            elif source in translations:
                text = convert(translations[source], english, source)
            else:
                sys.exit(f'{name}: no translation of "{source}"')
            messages.append((key, text))
        write(os.path.join(out, f'{code}.ftl'), messages)
        print(f'Wrote {code}.ftl, {len(messages)} messages.')


if __name__ == '__main__':
    main()
