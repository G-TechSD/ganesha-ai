//! Create a 5-page document about cats in LibreOffice Writer
//!
//! Run with: cargo run --example cats_document --features computer-use

use ganesha::input::{InputController, MouseButton};
use ganesha::vision::VisionController;
use std::time::Duration;
use tokio::time::sleep;

const CAT_CONTENT: &str = r#"The Wonderful World of Cats

Chapter 1: Introduction to Cats

Cats have been companions to humans for thousands of years, dating back to ancient Egypt where they were revered as sacred animals. These remarkable creatures have captured our hearts with their independent spirits, graceful movements, and mysterious personalities. Whether lounging in a sunny spot or stalking an invisible prey, cats bring joy and fascination to millions of households around the world.

The domestic cat, known scientifically as Felis catus, is a small carnivorous mammal that has evolved from wild ancestors to become one of the most popular pets globally. With over 70 distinct breeds recognized by various cat registries, these felines come in an astounding variety of colors, patterns, sizes, and temperaments.

Chapter 2: The History of Cats and Humans

The relationship between cats and humans began approximately 10,000 years ago in the Fertile Crescent region of the Middle East. As humans transitioned from nomadic hunting to agricultural settlements, grain stores attracted mice and other rodents. Wild cats, drawn by this abundant food source, began living near human communities. This mutually beneficial relationship marked the beginning of cat domestication.

Ancient Egyptians held cats in the highest regard. The goddess Bastet, depicted with the head of a cat, was worshipped as a deity of home, fertility, and protection. Killing a cat, even accidentally, was punishable by death in ancient Egypt. When a household cat died, families would shave their eyebrows in mourning and have the cat mummified.

Cats spread throughout the world via trade routes. Phoenician traders brought cats to Europe, where they were valued for their rodent-hunting abilities. During the Middle Ages, cats faced persecution in Europe due to superstitious beliefs linking them to witchcraft. This dark period led to mass killings of cats, which ironically contributed to the spread of the bubonic plague by allowing rat populations to flourish.

The Renaissance period saw a revival of appreciation for cats. They became popular subjects in art and literature, and their status as beloved companions was restored. Today, cats are the second most popular pet worldwide, with an estimated 600 million living in homes across the globe.

Chapter 3: Understanding Cat Behavior

Cats communicate through a complex system of vocalizations, body language, and scent marking. The familiar meow is primarily used to communicate with humans rather than other cats. Adult cats rarely meow at each other, reserving this vocalization for interactions with their human companions.

Purring is perhaps the most recognizable cat sound. While commonly associated with contentment, cats also purr when stressed, injured, or seeking comfort. Research suggests that the frequency of a cat's purr, between 25 and 150 Hertz, may promote healing and bone density.

A cat's tail is a remarkable indicator of mood. A tail held high signals confidence and happiness, while a puffed-up tail indicates fear or aggression. A slowly swishing tail often means the cat is focused on something, while rapid thrashing suggests irritation or excitement.

Cats are crepuscular animals, meaning they are most active during dawn and dusk. This behavior is inherited from their wild ancestors who hunted during these times to avoid larger predators and take advantage of prey activity patterns.

Chapter 4: Cat Health and Nutrition

Proper nutrition is essential for a cat's health and longevity. As obligate carnivores, cats require a diet high in animal protein. Unlike dogs, cats cannot synthesize certain essential nutrients from plant sources and must obtain them directly from meat.

Taurine, an amino acid found abundantly in animal tissue, is crucial for cat health. Deficiency can lead to serious conditions including heart disease, vision problems, and reproductive issues. Commercial cat foods are formulated to include adequate taurine levels.

Fresh water should always be available to cats. Many cats prefer running water, which explains the popularity of cat water fountains. Adequate hydration helps prevent urinary tract issues, which are common in cats, especially males.

Regular veterinary check-ups are important for maintaining cat health. Cats are masters at hiding illness, an evolutionary trait that protected them from predators in the wild. Annual examinations can catch health problems early when they are most treatable.

Dental health is often overlooked in cats. Periodontal disease affects the majority of cats over three years old. Regular dental care, including professional cleanings and at-home tooth brushing, can prevent painful dental conditions.

Chapter 5: Popular Cat Breeds

The Persian cat, with its luxurious long coat and flat face, is one of the oldest and most recognizable breeds. Known for their calm, gentle temperament, Persians make excellent indoor companions. Their beautiful coats require daily grooming to prevent matting.

Maine Coons are the largest domestic cat breed, with males sometimes weighing over 20 pounds. Despite their imposing size, they are known as gentle giants with friendly, sociable personalities. Their thick, water-resistant coats and tufted ears reflect their origins in the cold northeastern United States.

Siamese cats are famous for their striking blue eyes, pointed coloration, and vocal personalities. They are among the most intelligent and social cat breeds, often forming strong bonds with their owners and demanding attention.

The British Shorthair is known for its round face, dense coat, and calm demeanor. The blue variety, with its distinctive grey-blue fur and copper eyes, is particularly popular. These cats are adaptable and get along well with children and other pets.

Bengal cats bring a touch of the wild into homes with their leopard-like spotted or marbled coats. Despite their exotic appearance, they are domestic cats bred from Asian leopard cats. Bengals are highly active and intelligent, requiring plenty of mental and physical stimulation.

Chapter 6: Caring for Your Cat

Creating a safe, enriching environment is essential for a happy cat. Indoor cats live longer on average than outdoor cats, protected from traffic, predators, and diseases. However, indoor cats need environmental enrichment to prevent boredom.

Scratching is a natural behavior that serves multiple purposes: it removes dead outer layers of claws, stretches muscles, and marks territory. Providing appropriate scratching posts or pads redirects this behavior away from furniture.

Litter box maintenance is crucial for cat health and household harmony. The general rule is one litter box per cat plus one extra. Boxes should be scooped daily and completely cleaned weekly. Most cats prefer unscented, clumping litter.

Play is important for cats of all ages. Interactive play with toys that mimic prey behavior satisfies hunting instincts and provides exercise. Just 15 minutes of active play daily can help maintain a healthy weight and prevent behavioral problems.

Grooming needs vary by breed. While short-haired cats are generally low maintenance, long-haired breeds require daily brushing. Regular grooming also provides an opportunity to check for lumps, skin problems, or parasites.

Chapter 7: The Science of Cat Cognition

Recent research has revealed that cats are far more intelligent than previously believed. They possess excellent memories, capable of retaining information for up to 10 years. Cats can learn through observation and have demonstrated problem-solving abilities.

Cats recognize their names but often choose not to respond. Studies have shown that cats can distinguish their names from similar-sounding words. This selective response is consistent with their independent nature rather than a lack of understanding.

Cats form attachments to their owners similar to the bonds between dogs and humans, or between infants and parents. Research using the Secure Base Test has shown that cats display attachment behaviors and use their owners as a source of security.

Spatial memory in cats is highly developed. They create mental maps of their environment and can navigate complex routes. This ability served their ancestors well when hunting across large territories.

Chapter 8: Cats in Culture and Art

Throughout history, cats have inspired artists, writers, and musicians. From ancient Egyptian sculptures to modern internet memes, cats have maintained a prominent place in human culture.

Famous cats in literature include the Cheshire Cat from Lewis Carroll's Alice's Adventures in Wonderland, the Cat in the Hat by Dr. Seuss, and Behemoth from Mikhail Bulgakov's The Master and Margarita. These fictional felines capture various aspects of cat nature, from mystery to mischief.

The internet age has elevated cats to unprecedented cultural prominence. Grumpy Cat, Lil Bub, and countless other feline celebrities have amassed millions of followers. Cat videos consistently rank among the most viewed content online.

Japanese culture has a particularly strong association with cats. The maneki-neko, or beckoning cat figurine, is a common good luck charm. Cat cafes, originating in Taiwan but popularized in Japan, have spread worldwide, allowing cat lovers to enjoy feline company while having coffee.

Conclusion

Cats continue to captivate us with their beauty, mystery, and companionship. Whether you prefer a cuddly lap cat or an adventurous explorer, there is a feline friend suited to every lifestyle. As our understanding of cats grows through scientific research, so does our appreciation for these remarkable animals.

The bond between humans and cats, forged thousands of years ago, remains strong today. By providing proper care, nutrition, and love, we can ensure our feline companions live long, healthy, happy lives. In return, they offer us affection, entertainment, and a daily reminder of the natural world's elegance and wonder.

Thank you for reading this comprehensive guide to the wonderful world of cats."#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║           GANESHA: CREATE CAT DOCUMENT                        ║");
    println!("║           Close Firefox -> Open Writer -> 5 Pages             ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let vision = VisionController::new();
    let input = InputController::new();

    println!("[*] Enabling modules...");
    vision.enable()?;
    input.enable()?;
    println!("[✓] Ready\n");

    // Step 1: Close Firefox by clicking the X button (top-right of window)
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 1: Close Firefox");
    println!("═══════════════════════════════════════════════════════════════");
    // GNOME window close button is typically at top-right
    // For a maximized window on 1920x1080, close button is around (1900, 14)
    println!("[*] Clicking close button (1900, 14)...");
    input.mouse_move(1900, 14)?;
    input.mouse_click(MouseButton::Left)?;
    sleep(Duration::from_secs(1)).await;
    println!("[✓] Firefox closed\n");

    // Step 2: Open LibreOffice Writer
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 2: Open LibreOffice Writer");
    println!("═══════════════════════════════════════════════════════════════");

    println!("[*] Clicking Activities...");
    input.mouse_move(50, 14)?;
    input.mouse_click(MouseButton::Left)?;
    sleep(Duration::from_millis(800)).await;

    println!("[*] Typing 'writer'...");
    input.type_text("writer")?;
    sleep(Duration::from_millis(1000)).await;

    println!("[*] Pressing Enter to launch...");
    input.key_press("Return")?;
    sleep(Duration::from_secs(4)).await;  // Writer takes a moment to load
    println!("[✓] LibreOffice Writer launched\n");

    // Step 3: Type the document
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 3: Writing 5-page document about cats");
    println!("═══════════════════════════════════════════════════════════════");

    // Split content into chunks and type with progress
    let chunks: Vec<&str> = CAT_CONTENT.split("\n\n").collect();
    let total = chunks.len();

    for (i, chunk) in chunks.iter().enumerate() {
        println!("[*] Typing paragraph {}/{}...", i + 1, total);

        // Type the chunk
        input.type_text(chunk)?;

        // Add paragraph breaks
        input.key_press("Return")?;
        input.key_press("Return")?;

        // Small delay between paragraphs
        sleep(Duration::from_millis(100)).await;
    }

    println!("[✓] Document content typed\n");

    // Step 4: Save the document
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 4: Save document");
    println!("═══════════════════════════════════════════════════════════════");

    println!("[*] Pressing Ctrl+S to save...");
    input.key_combination("ctrl+s")?;
    sleep(Duration::from_secs(2)).await;

    // Type filename in save dialog
    println!("[*] Typing filename 'cats_document'...");
    input.type_text("cats_document")?;
    sleep(Duration::from_millis(300)).await;

    println!("[*] Pressing Enter to save...");
    input.key_press("Return")?;
    sleep(Duration::from_secs(2)).await;
    println!("[✓] Document saved\n");

    // Capture final screenshot
    println!("═══════════════════════════════════════════════════════════════");
    println!("STEP 5: Capture result");
    println!("═══════════════════════════════════════════════════════════════");

    let screenshot = vision.capture_screen()?;
    println!("[✓] Screenshot: {}x{}", screenshot.width, screenshot.height);

    // Save screenshot
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&screenshot.data)?;
    std::fs::write("/tmp/ganesha_cats_doc.png", &bytes)?;
    println!("[✓] Saved: /tmp/ganesha_cats_doc.png");

    vision.disable();
    input.disable();

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  DONE - 5 page cat document created in LibreOffice Writer!    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}
