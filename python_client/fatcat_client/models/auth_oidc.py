# coding: utf-8

"""
    fatcat

    A scalable, versioned, API-oriented catalog of bibliographic entities and file metadata  # noqa: E501

    OpenAPI spec version: 0.3.0
    
    Generated by: https://github.com/swagger-api/swagger-codegen.git
"""


import pprint
import re  # noqa: F401

import six


class AuthOidc(object):
    """NOTE: This class is auto generated by the swagger code generator program.

    Do not edit the class manually.
    """

    """
    Attributes:
      swagger_types (dict): The key is attribute name
                            and the value is attribute type.
      attribute_map (dict): The key is attribute name
                            and the value is json key in definition.
    """
    swagger_types = {
        'provider': 'str',
        'sub': 'str',
        'iss': 'str',
        'preferred_username': 'str'
    }

    attribute_map = {
        'provider': 'provider',
        'sub': 'sub',
        'iss': 'iss',
        'preferred_username': 'preferred_username'
    }

    def __init__(self, provider=None, sub=None, iss=None, preferred_username=None):  # noqa: E501
        """AuthOidc - a model defined in Swagger"""  # noqa: E501

        self._provider = None
        self._sub = None
        self._iss = None
        self._preferred_username = None
        self.discriminator = None

        self.provider = provider
        self.sub = sub
        self.iss = iss
        self.preferred_username = preferred_username

    @property
    def provider(self):
        """Gets the provider of this AuthOidc.  # noqa: E501


        :return: The provider of this AuthOidc.  # noqa: E501
        :rtype: str
        """
        return self._provider

    @provider.setter
    def provider(self, provider):
        """Sets the provider of this AuthOidc.


        :param provider: The provider of this AuthOidc.  # noqa: E501
        :type: str
        """
        if provider is None:
            raise ValueError("Invalid value for `provider`, must not be `None`")  # noqa: E501

        self._provider = provider

    @property
    def sub(self):
        """Gets the sub of this AuthOidc.  # noqa: E501


        :return: The sub of this AuthOidc.  # noqa: E501
        :rtype: str
        """
        return self._sub

    @sub.setter
    def sub(self, sub):
        """Sets the sub of this AuthOidc.


        :param sub: The sub of this AuthOidc.  # noqa: E501
        :type: str
        """
        if sub is None:
            raise ValueError("Invalid value for `sub`, must not be `None`")  # noqa: E501

        self._sub = sub

    @property
    def iss(self):
        """Gets the iss of this AuthOidc.  # noqa: E501


        :return: The iss of this AuthOidc.  # noqa: E501
        :rtype: str
        """
        return self._iss

    @iss.setter
    def iss(self, iss):
        """Sets the iss of this AuthOidc.


        :param iss: The iss of this AuthOidc.  # noqa: E501
        :type: str
        """
        if iss is None:
            raise ValueError("Invalid value for `iss`, must not be `None`")  # noqa: E501

        self._iss = iss

    @property
    def preferred_username(self):
        """Gets the preferred_username of this AuthOidc.  # noqa: E501


        :return: The preferred_username of this AuthOidc.  # noqa: E501
        :rtype: str
        """
        return self._preferred_username

    @preferred_username.setter
    def preferred_username(self, preferred_username):
        """Sets the preferred_username of this AuthOidc.


        :param preferred_username: The preferred_username of this AuthOidc.  # noqa: E501
        :type: str
        """
        if preferred_username is None:
            raise ValueError("Invalid value for `preferred_username`, must not be `None`")  # noqa: E501

        self._preferred_username = preferred_username

    def to_dict(self):
        """Returns the model properties as a dict"""
        result = {}

        for attr, _ in six.iteritems(self.swagger_types):
            value = getattr(self, attr)
            if isinstance(value, list):
                result[attr] = list(map(
                    lambda x: x.to_dict() if hasattr(x, "to_dict") else x,
                    value
                ))
            elif hasattr(value, "to_dict"):
                result[attr] = value.to_dict()
            elif isinstance(value, dict):
                result[attr] = dict(map(
                    lambda item: (item[0], item[1].to_dict())
                    if hasattr(item[1], "to_dict") else item,
                    value.items()
                ))
            else:
                result[attr] = value

        return result

    def to_str(self):
        """Returns the string representation of the model"""
        return pprint.pformat(self.to_dict())

    def __repr__(self):
        """For `print` and `pprint`"""
        return self.to_str()

    def __eq__(self, other):
        """Returns true if both objects are equal"""
        if not isinstance(other, AuthOidc):
            return False

        return self.__dict__ == other.__dict__

    def __ne__(self, other):
        """Returns true if both objects are not equal"""
        return not self == other
