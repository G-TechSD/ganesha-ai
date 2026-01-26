document.addEventListener('DOMContentLoaded', function() {
    const messageInput = document.getElementById('message-input');
    const sendButton = document.getElementById('send-button');
    const chatMessages = document.querySelector('.chat-messages');

    sendButton.addEventListener('click', function() {
        const messageText = messageInput.value;
        if (messageText.trim() !== '') {
            const messageElement = document.createElement('div');
            messageElement.classList.add('message', 'sent');
            messageElement.textContent = messageText;
            chatMessages.appendChild(messageElement);
            messageInput.value = '';

            // Simulate a received message (replace with actual AI response)
            setTimeout(function() {
                const responseElement = document.createElement('div');
                responseElement.classList.add('message', 'received');
                responseElement.textContent = 'This is a simulated response.';
                chatMessages.appendChild(responseElement);
                chatMessages.scrollTop = chatMessages.scrollHeight; // Scroll to bottom
            }, 500);

            chatMessages.scrollTop = chatMessages.scrollHeight; // Scroll to bottom
        }
    });

    messageInput.addEventListener('keypress', function(event) {
        if (event.key === 'Enter') {
            sendButton.click();
        }
    });

    // Scroll to bottom on load
    chatMessages.scrollTop = chatMessages.scrollHeight;
});
