//! Vocabulario en español de Chile: letras, palabras con dibujo, animales
//! con su sonido, colores, formas y frases de ánimo.
//!
//! Para agregar palabras basta con editar las tablas de este archivo.

use crate::math::{hex, Rgba};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    M,
    F,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Word {
    pub word: &'static str,
    pub plural: &'static str,
    pub emoji: &'static str,
    pub gender: Gender,
    /// Sonido del animal ("" si no tiene).
    pub sound: &'static str,
}

impl Word {
    /// "un perro" / "una vaca"
    pub fn with_article(&self) -> String {
        match self.gender {
            Gender::M => format!("un {}", self.word),
            Gender::F => format!("una {}", self.word),
        }
    }

    pub fn first_letter(&self) -> char {
        let c = self.word.chars().next().unwrap_or('a');
        fold_letter(c).unwrap_or('A')
    }
}

const fn m(word: &'static str, plural: &'static str, emoji: &'static str) -> Word {
    Word {
        word,
        plural,
        emoji,
        gender: Gender::M,
        sound: "",
    }
}
const fn f(word: &'static str, plural: &'static str, emoji: &'static str) -> Word {
    Word {
        word,
        plural,
        emoji,
        gender: Gender::F,
        sound: "",
    }
}
const fn ma(
    word: &'static str,
    plural: &'static str,
    emoji: &'static str,
    sound: &'static str,
) -> Word {
    Word {
        word,
        plural,
        emoji,
        gender: Gender::M,
        sound,
    }
}
const fn fa(
    word: &'static str,
    plural: &'static str,
    emoji: &'static str,
    sound: &'static str,
) -> Word {
    Word {
        word,
        plural,
        emoji,
        gender: Gender::F,
        sound,
    }
}

pub struct Letter {
    pub upper: &'static str,
    /// Cómo se dice la letra en voz alta.
    pub name: &'static str,
    /// Palabras de ejemplo. La primera NO tiene por qué empezar con la letra
    /// sólo en casos raros (X), donde se dice "como en ...".
    pub words: &'static [Word],
    /// true si las palabras sólo contienen la letra (X, W).
    pub contains_only: bool,
}

pub const PERRO: Word = ma("perro", "perros", "🐶", "¡Guau, guau!");
pub const GATO: Word = ma("gato", "gatos", "🐱", "¡Miau!");
pub const VACA: Word = fa("vaca", "vacas", "🐮", "¡Muuuu!");
pub const PATO: Word = ma("pato", "patos", "🦆", "¡Cuac, cuac!");
pub const OVEJA: Word = fa("oveja", "ovejas", "🐑", "¡Beeee!");
pub const CHANCHO: Word = ma("chancho", "chanchos", "🐷", "¡Oinc, oinc!");
pub const GALLO: Word = ma("gallo", "gallos", "🐓", "¡Cocorocó!");
pub const POLLITO: Word = ma("pollito", "pollitos", "🐥", "¡Pío, pío!");
pub const CABALLO: Word = ma("caballo", "caballos", "🐴", "¡Iiiiiih!");
pub const RANA: Word = fa("rana", "ranas", "🐸", "¡Croac, croac!");
pub const LEON: Word = ma("león", "leones", "🦁", "¡Grrrr!");
pub const MONO: Word = ma("mono", "monos", "🐵", "¡U, u, a, a!");
pub const ELEFANTE: Word = ma("elefante", "elefantes", "🐘", "¡Pruuuu!");
pub const RATON: Word = ma("ratón", "ratones", "🐭", "¡Iii, iii!");
pub const ABEJA: Word = fa("abeja", "abejas", "🐝", "¡Bzzzz!");
pub const BUHO: Word = ma("búho", "búhos", "🦉", "¡Uh, uh!");
pub const SERPIENTE: Word = fa("serpiente", "serpientes", "🐍", "¡Sssss!");
pub const TIGRE: Word = ma("tigre", "tigres", "🐯", "¡Grrrr!");
pub const OSO: Word = ma("oso", "osos", "🐻", "¡Grrr!");
pub const LOBO: Word = ma("lobo", "lobos", "🐺", "¡Auuuuu!");
pub const DINOSAURIO: Word = ma("dinosaurio", "dinosaurios", "🦖", "¡Roaaar!");
pub const PINGUINO: Word = ma("pingüino", "pingüinos", "🐧", "");
pub const BALLENA: Word = fa("ballena", "ballenas", "🐳", "¡Splash!");
pub const PEZ: Word = ma("pez", "peces", "🐟", "¡Glu, glu!");
pub const CONEJO: Word = ma("conejo", "conejos", "🐰", "");
pub const JIRAFA: Word = fa("jirafa", "jirafas", "🦒", "");
pub const TORTUGA: Word = fa("tortuga", "tortugas", "🐢", "");
pub const MARIPOSA: Word = fa("mariposa", "mariposas", "🦋", "");
pub const ZORRO: Word = ma("zorro", "zorros", "🦊", "");
pub const KOALA: Word = ma("koala", "koalas", "🐨", "");
pub const DELFIN: Word = ma("delfín", "delfines", "🐬", "");
pub const TREN: Word = ma("tren", "trenes", "🚂", "¡Chu, chu!");
pub const AUTO: Word = ma("auto", "autos", "🚗", "¡Pip, pip!");

pub const MANZANA: Word = f("manzana", "manzanas", "🍎");
pub const FRUTILLA: Word = f("frutilla", "frutillas", "🍓");
pub const ESTRELLA: Word = f("estrella", "estrellas", "⭐");
pub const PELOTA: Word = f("pelota", "pelotas", "⚽");
pub const FLOR: Word = f("flor", "flores", "🌸");
pub const GLOBO: Word = m("globo", "globos", "🎈");
pub const PLATANO: Word = m("plátano", "plátanos", "🍌");
pub const UVA: Word = f("uva", "uvas", "🍇");
pub const PALTA: Word = f("palta", "paltas", "🥑");
pub const SANDIA: Word = f("sandía", "sandías", "🍉");

pub static LETTERS: &[Letter] = &[
    Letter {
        upper: "A",
        name: "a",
        contains_only: false,
        words: &[
            m("avión", "aviones", "✈️"),
            ABEJA,
            m("arcoíris", "arcoíris", "🌈"),
        ],
    },
    Letter {
        upper: "B",
        name: "be",
        contains_only: false,
        words: &[
            BALLENA,
            m("barco", "barcos", "⛵"),
            BUHO,
            f("bicicleta", "bicicletas", "🚲"),
        ],
    },
    Letter {
        upper: "C",
        name: "ce",
        contains_only: false,
        words: &[
            CONEJO,
            CABALLO,
            CHANCHO,
            m("cohete", "cohetes", "🚀"),
            f("casa", "casas", "🏠"),
        ],
    },
    Letter {
        upper: "D",
        name: "de",
        contains_only: false,
        words: &[DINOSAURIO, DELFIN, m("durazno", "duraznos", "🍑")],
    },
    Letter {
        upper: "E",
        name: "e",
        contains_only: false,
        words: &[ELEFANTE, ESTRELLA, m("erizo", "erizos", "🦔")],
    },
    Letter {
        upper: "F",
        name: "efe",
        contains_only: false,
        words: &[FRUTILLA, FLOR, m("fuego", "fuegos", "🔥")],
    },
    Letter {
        upper: "G",
        name: "ge",
        contains_only: false,
        words: &[GATO, GLOBO, GALLO, f("guitarra", "guitarras", "🎸")],
    },
    Letter {
        upper: "H",
        name: "hache",
        contains_only: false,
        words: &[
            m("helado", "helados", "🍦"),
            f("hormiga", "hormigas", "🐜"),
            m("huevo", "huevos", "🥚"),
            m("hongo", "hongos", "🍄"),
        ],
    },
    Letter {
        upper: "I",
        name: "i",
        contains_only: false,
        words: &[
            f("iguana", "iguanas", "🦎"),
            f("isla", "islas", "🏝️"),
            m("imán", "imanes", "🧲"),
        ],
    },
    Letter {
        upper: "J",
        name: "jota",
        contains_only: false,
        words: &[JIRAFA, m("jugo", "jugos", "🧃")],
    },
    Letter {
        upper: "K",
        name: "ka",
        contains_only: false,
        words: &[KOALA, m("kiwi", "kiwis", "🥝")],
    },
    Letter {
        upper: "L",
        name: "ele",
        contains_only: false,
        words: &[
            LEON,
            f("luna", "lunas", "🌙"),
            m("limón", "limones", "🍋"),
            LOBO,
        ],
    },
    Letter {
        upper: "M",
        name: "eme",
        contains_only: false,
        words: &[MARIPOSA, MANZANA, MONO],
    },
    Letter {
        upper: "N",
        name: "ene",
        contains_only: false,
        words: &[
            f("nube", "nubes", "☁️"),
            f("naranja", "naranjas", "🍊"),
            f("nariz", "narices", "👃"),
        ],
    },
    Letter {
        upper: "Ñ",
        name: "eñe",
        contains_only: false,
        words: &[m("ñandú", "ñandúes", "🐦")],
    },
    Letter {
        upper: "O",
        name: "o",
        contains_only: false,
        words: &[OSO, OVEJA, f("ola", "olas", "🌊")],
    },
    Letter {
        upper: "P",
        name: "pe",
        contains_only: false,
        words: &[PERRO, PATO, PEZ, PELOTA, POLLITO, PALTA, PINGUINO],
    },
    Letter {
        upper: "Q",
        name: "cu",
        contains_only: false,
        words: &[m("queso", "quesos", "🧀")],
    },
    Letter {
        upper: "R",
        name: "erre",
        contains_only: false,
        words: &[
            RATON,
            RANA,
            m("regalo", "regalos", "🎁"),
            m("reloj", "relojes", "⏰"),
        ],
    },
    Letter {
        upper: "S",
        name: "ese",
        contains_only: false,
        words: &[m("sol", "soles", "☀️"), SERPIENTE, SANDIA],
    },
    Letter {
        upper: "T",
        name: "te",
        contains_only: false,
        words: &[
            TORTUGA,
            TREN,
            TIGRE,
            m("tomate", "tomates", "🍅"),
            m("tambor", "tambores", "🥁"),
        ],
    },
    Letter {
        upper: "U",
        name: "u",
        contains_only: false,
        words: &[m("unicornio", "unicornios", "🦄"), UVA],
    },
    Letter {
        upper: "V",
        name: "ve",
        contains_only: false,
        words: &[
            VACA,
            m("volcán", "volcanes", "🌋"),
            m("violín", "violines", "🎻"),
        ],
    },
    Letter {
        upper: "W",
        name: "doble ve",
        contains_only: true,
        words: &[m("kiwi", "kiwis", "🥝"), m("wafle", "wafles", "🧇")],
    },
    Letter {
        upper: "X",
        name: "equis",
        contains_only: true,
        words: &[m("taxi", "taxis", "🚕"), m("saxofón", "saxofones", "🎷")],
    },
    Letter {
        upper: "Y",
        name: "i griega",
        contains_only: false,
        words: &[m("yoyó", "yoyós", "🪀"), m("yate", "yates", "🛥️")],
    },
    Letter {
        upper: "Z",
        name: "zeta",
        contains_only: false,
        words: &[
            ZORRO,
            f("zanahoria", "zanahorias", "🥕"),
            f("zapatilla", "zapatillas", "👟"),
        ],
    },
];

/// Animales para las burbujas (todos con sonido).
pub static ANIMALS: &[Word] = &[
    PERRO, GATO, VACA, PATO, OVEJA, CHANCHO, GALLO, POLLITO, CABALLO, RANA, LEON, MONO, ELEFANTE,
    RATON, ABEJA, BUHO, SERPIENTE, TIGRE, OSO, LOBO, DINOSAURIO, BALLENA, PEZ, TREN, AUTO,
];

/// Cosas para contar con los números.
pub static COUNTABLES: &[Word] = &[
    MANZANA, FRUTILLA, ESTRELLA, PELOTA, FLOR, GLOBO, PATO, PERRO, GATO, PEZ, MARIPOSA, AUTO,
    PLATANO, UVA, PALTA,
];

/// Palabras para el juego Escribir, de las más cortas a las más largas.
pub static WRITE_WORDS: &[Word] = &[
    m("sol", "soles", "☀️"),
    OSO,
    PEZ,
    UVA,
    f("mamá", "mamás", "👩"),
    m("papá", "papás", "👨"),
    GATO,
    PATO,
    VACA,
    f("casa", "casas", "🏠"),
    f("luna", "lunas", "🌙"),
    MONO,
    RANA,
    AUTO,
    TREN,
    FLOR,
    LEON,
    PERRO,
    PALTA,
    ABEJA,
];

pub static NUMBERS: [&str; 11] = [
    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez",
];

/// "una flor", "dos flores"... (concordancia de "uno" con el género)
pub fn count_phrase(n: u32, w: &Word) -> String {
    match n {
        0 => format!("cero {}", w.plural),
        1 => w.with_article(),
        n if (n as usize) < NUMBERS.len() => format!("{} {}", NUMBERS[n as usize], w.plural),
        n => format!("{} {}", n, w.plural),
    }
}

pub fn letter_info(c: char) -> Option<&'static Letter> {
    let mut buf = [0u8; 4];
    let s: &str = c.encode_utf8(&mut buf);
    LETTERS.iter().find(|l| l.upper == s)
}

/// Pasa una tecla a letra mayúscula sin tilde (á -> A, ñ -> Ñ).
pub fn fold_letter(c: char) -> Option<char> {
    let c = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ñ' | 'Ñ' => 'Ñ',
        'ç' | 'Ç' => 'C',
        c if c.is_ascii_alphabetic() => c.to_ascii_uppercase(),
        _ => return None,
    };
    Some(c)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorName {
    pub m: &'static str,
    pub f: &'static str,
    pub rgb: Rgba,
}

impl ColorName {
    pub fn for_gender(&self, g: Gender) -> &'static str {
        match g {
            Gender::M => self.m,
            Gender::F => self.f,
        }
    }
}

const fn cn(m: &'static str, f: &'static str, rgb: u32) -> ColorName {
    ColorName {
        m,
        f,
        rgb: hex(rgb),
    }
}

pub const ROJO: ColorName = cn("rojo", "roja", 0xE53935);
pub const NARANJO: ColorName = cn("naranjo", "naranja", 0xFB8C00);
pub const AMARILLO: ColorName = cn("amarillo", "amarilla", 0xFDD835);
pub const VERDE: ColorName = cn("verde", "verde", 0x43A047);
pub const CELESTE: ColorName = cn("celeste", "celeste", 0x4FC3F7);
pub const AZUL: ColorName = cn("azul", "azul", 0x1E5BD8);
pub const MORADO: ColorName = cn("morado", "morada", 0x8E24AA);
pub const ROSADO: ColorName = cn("rosado", "rosada", 0xF06292);
pub const CAFE: ColorName = cn("café", "café", 0x795548);
pub const NEGRO: ColorName = cn("negro", "negra", 0x263238);
pub const BLANCO: ColorName = cn("blanco", "blanca", 0xFFFFFF);

/// Colores bien distintos entre sí, para el juego de formas.
pub static GAME_COLORS: &[ColorName] = &[ROJO, AZUL, AMARILLO, VERDE, NARANJO, MORADO, ROSADO];

/// Paleta del modo Pintar.
pub static PAINT_COLORS: &[ColorName] = &[
    ROJO, NARANJO, AMARILLO, VERDE, CELESTE, AZUL, MORADO, ROSADO, CAFE, NEGRO,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeName {
    pub name: &'static str,
    pub gender: Gender,
    pub shape: crate::gfx::Shape,
}

pub static SHAPES: &[ShapeName] = &[
    ShapeName {
        name: "círculo",
        gender: Gender::M,
        shape: crate::gfx::Shape::Circle,
    },
    ShapeName {
        name: "cuadrado",
        gender: Gender::M,
        shape: crate::gfx::Shape::RoundRect,
    },
    ShapeName {
        name: "triángulo",
        gender: Gender::M,
        shape: crate::gfx::Shape::Triangle,
    },
    ShapeName {
        name: "estrella",
        gender: Gender::F,
        shape: crate::gfx::Shape::Star,
    },
    ShapeName {
        name: "corazón",
        gender: Gender::M,
        shape: crate::gfx::Shape::Heart,
    },
];

/// "el círculo rojo" / "la estrella roja"
pub fn shape_phrase(s: &ShapeName, c: &ColorName) -> String {
    let art = match s.gender {
        Gender::M => "el",
        Gender::F => "la",
    };
    format!("{} {} {}", art, s.name, c.for_gender(s.gender))
}

pub static PRAISE: &[&str] = &[
    "¡Muy bien!",
    "¡Bacán!",
    "¡Bravo!",
    "¡Excelente!",
    "¡Lo encontraste!",
    "¡Genial!",
    "¡Súper!",
];

/// Nombre hablado de teclas especiales (según `KeyboardEvent.code`).
pub fn key_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "Space" => "¡Espacio!",
        "Enter" | "NumpadEnter" => "¡Enter!",
        "Backspace" => "¡Borrar!",
        "Delete" => "¡Suprimir!",
        "Tab" => "¡Tab!",
        "ShiftLeft" | "ShiftRight" => "¡Shift!",
        "ControlLeft" | "ControlRight" => "¡Control!",
        "AltLeft" | "AltRight" => "¡Alt!",
        "MetaLeft" | "MetaRight" => "¡Windows!",
        "CapsLock" => "¡Mayúsculas!",
        "Escape" => "¡Escape!",
        "ArrowUp" => "¡Arriba!",
        "ArrowDown" => "¡Abajo!",
        "ArrowLeft" => "¡Izquierda!",
        "ArrowRight" => "¡Derecha!",
        "Home" => "¡Inicio!",
        "End" => "¡Fin!",
        "PageUp" => "¡Página arriba!",
        "PageDown" => "¡Página abajo!",
        "Insert" => "¡Insertar!",
        "ContextMenu" => "¡Menú!",
        "F1" => "¡Efe uno!",
        "F2" => "¡Efe dos!",
        "F3" => "¡Efe tres!",
        "F4" => "¡Efe cuatro!",
        "F5" => "¡Efe cinco!",
        "F6" => "¡Efe seis!",
        "F7" => "¡Efe siete!",
        "F8" => "¡Efe ocho!",
        "F9" => "¡Efe nueve!",
        "F10" => "¡Efe diez!",
        "F11" => "¡Efe once!",
        "F12" => "¡Efe doce!",
        _ => return None,
    })
}

/// Signos que se muestran grandes y se nombran.
pub fn symbol_name(c: char) -> Option<(&'static str, &'static str)> {
    Some(match c {
        '.' => (".", "punto"),
        ',' => (",", "coma"),
        ';' => (";", "punto y coma"),
        ':' => (":", "dos puntos"),
        '-' => ("-", "menos"),
        '+' => ("+", "más"),
        '*' => ("*", "asterisco"),
        '/' => ("/", "barra"),
        '=' => ("=", "igual"),
        '?' => ("?", "signo de pregunta"),
        '¿' => ("¿", "signo de pregunta"),
        '!' => ("!", "signo de exclamación"),
        '¡' => ("¡", "signo de exclamación"),
        '@' => ("@", "arroba"),
        '#' => ("#", "gato"),
        '$' => ("$", "peso"),
        '%' => ("%", "por ciento"),
        '&' => ("&", "y"),
        '(' => ("(", "paréntesis"),
        ')' => (")", "paréntesis"),
        '<' => ("<", "menor que"),
        '>' => (">", "mayor que"),
        '"' => ("\"", "comillas"),
        '\'' => ("'", "apóstrofo"),
        '_' => ("_", "guion bajo"),
        '{' => ("{", "llave"),
        '}' => ("}", "llave"),
        '[' => ("[", "corchete"),
        ']' => ("]", "corchete"),
        '|' => ("|", "barra"),
        '°' => ("°", "grado"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_letter_has_words() {
        assert_eq!(LETTERS.len(), 27);
        for l in LETTERS {
            assert!(!l.words.is_empty(), "{} sin palabras", l.upper);
            if !l.contains_only {
                for w in l.words {
                    let first = w.first_letter().to_string();
                    assert_eq!(first, l.upper, "{} no empieza con {}", w.word, l.upper);
                }
            }
        }
    }

    #[test]
    fn letters_cover_keyboard() {
        for c in "ABCDEFGHIJKLMNÑOPQRSTUVWXYZ".chars() {
            assert!(letter_info(c).is_some(), "falta {c}");
        }
    }

    #[test]
    fn folding() {
        assert_eq!(fold_letter('á'), Some('A'));
        assert_eq!(fold_letter('ñ'), Some('Ñ'));
        assert_eq!(fold_letter('z'), Some('Z'));
        assert_eq!(fold_letter('3'), None);
    }

    #[test]
    fn grammar() {
        assert_eq!(count_phrase(1, &FLOR), "una flor");
        assert_eq!(count_phrase(3, &FLOR), "tres flores");
        assert_eq!(count_phrase(1, &PERRO), "un perro");
        assert_eq!(shape_phrase(&SHAPES[3], &ROJO), "la estrella roja");
        assert_eq!(shape_phrase(&SHAPES[0], &NARANJO), "el círculo naranjo");
    }

    #[test]
    fn animals_have_sounds() {
        for a in ANIMALS {
            assert!(!a.sound.is_empty(), "{} sin sonido", a.word);
        }
    }
}
