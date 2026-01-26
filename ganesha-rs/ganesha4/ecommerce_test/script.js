document.addEventListener('DOMContentLoaded', function() {
    const quantityInput = document.getElementById('quantity');
    const quantityBtns = document.querySelectorAll('.quantity-btn');
    const addToCartBtn = document.querySelector('.add-to-cart-btn');
    const imageGallery = document.querySelector('.product-image-gallery');

    quantityBtns.forEach(btn => {
        btn.addEventListener('click', function() {
            let action = this.dataset.action;
            let currentValue = parseInt(quantityInput.value);

            if (action === 'increase') {
                quantityInput.value = currentValue + 1;
            } else if (action === 'decrease' && currentValue > 1) {
                quantityInput.value = currentValue - 1;
            }
        });
    });

    addToCartBtn.addEventListener('click', function() {
        alert('Added to cart!');
    });

    imageGallery.addEventListener('click', function(e) {
        if (e.target.tagName === 'IMG') {
            // Remove active class from all images
            imageGallery.querySelectorAll('img').forEach(img => img.classList.remove('active'));

            // Add active class to the clicked image
            e.target.classList.add('active');
        }
    });
});
