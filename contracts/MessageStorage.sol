pragma solidity ^0.8.29;


contract MessageStorage {
    address public owner;
    string[] public messages;

    event MessageWritten(string message, address indexed sender);

    modifier onlyOwner() {
        require(msg.sender == owner, "Only the owner can perform this action.");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    function writeMessage(string memory _message) public onlyOwner {
        messages.push(_message);
        emit MessageWritten(_message, msg.sender);
    }

    function getMessages() public view returns (string[] memory) {
        return messages;
    }
}
