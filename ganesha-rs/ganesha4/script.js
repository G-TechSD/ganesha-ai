document.addEventListener('DOMContentLoaded', function() {
  const inputText = document.getElementById('inputText');
  const checkButton = document.getElementById('checkButton');
  const result = document.getElementById('result');

  checkButton.addEventListener('click', function() {
    const text = inputText.value;
    const cleanText = text.toLowerCase().replace(/[^a-z0-9]/g, '');
    const reversedText = cleanText.split('').reverse().join('');

    if (cleanText === reversedText) {
      result.textContent = '"' + text + '" is a palindrome!';
    } else {
      result.textContent = '"' + text + '" is not a palindrome.';
    }
  });
});
